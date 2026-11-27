-- Runs inside PCSX-Redux to carry out one run of `ventris-workbench ps1-probe`.
--
-- The workbench starts the emulator with two scripts: first one it writes for the run, which sets the
-- global PLAN, then this one. PLAN says what to do at which frame: buttons to press, screenshots,
-- memory to dump, a state to save, ranges of memory to watch and addresses to log. Everything is
-- written to files; the emulator quits at the last frame. See ps1probe.py for what each part of the
-- plan holds.
--
-- Frame 0 is the first frame of the run: the first time the GPU reaches vertical blank after power-on,
-- or, when the run starts from a saved state, the moment that state has been loaded, which shows the
-- picture it was saved with. Each vertical blank after that is the next frame.

local plan = PLAN
local pad = PCSX.SIO0.slots[1].pads[1]
local memory = ffi.cast('uint8_t*', PCSX.getMemPtr())
local scratch = ffi.cast('uint8_t*', PCSX.getScratchPtr())
local rom = ffi.cast('uint8_t*', PCSX.getRomPtr())

local now = nil -- the frame of the run, nil until it starts
local loading = false
local finished = false
local logs = {} -- files the watches and logged addresses write to, by name
local breakpoints = {} -- kept so that they are not collected

local NAMES = {
    'at', 'v0', 'v1', 'a0', 'a1', 'a2', 'a3', 't0', 't1', 't2', 't3', 't4', 't5', 't6', 't7',
    's0', 's1', 's2', 's3', 's4', 's5', 's6', 's7', 't8', 't9', 'k0', 'k1', 'gp', 'sp', 'fp', 'ra',
}
-- The registers a line of a watch carries: arguments, results, stack and return address.
local SHORT = { 4, 5, 6, 7, 2, 3, 29, 31 }

local function hex(n) return bit.tohex(n, 8) end

local function finish(code, message)
    if finished then return end
    finished = true
    for _, file in pairs(logs) do file:close() end
    if message then print('ps1probe error: ' .. message) end
    if code == 0 then
        local done = io.open(plan.work .. '/done', 'wb')
        done:write('done')
        done:close()
    end
    PCSX.quit(code)
end

-- Every callback runs through this, so that an error ends the run with its message instead of
-- leaving the emulator running with nothing to stop it.
local function guarded(fn)
    return function(...)
        if finished then return end
        local ok, err = pcall(fn, ...)
        if not ok then finish(2, tostring(err)) end
    end
end

local function write(path, data)
    local file = assert(io.open(path, 'wb'))
    file:write(data)
    file:close()
end

-- A byte of what the processor sees at an address: main RAM (and its mirrors), the scratchpad or the
-- BIOS. Nil elsewhere, such as the hardware registers.
local function byteAt(address)
    local physical = bit.band(address, 0x1fffffff)
    if physical < 0x800000 then return memory[bit.band(physical, 0x1fffff)] end
    if physical >= 0x1f800000 and physical < 0x1f800400 then return scratch[physical - 0x1f800000] end
    if physical >= 0x1fc00000 and physical < 0x1fc80000 then return rom[physical - 0x1fc00000] end
    return nil
end

local function valueAt(address, width)
    local value = 0
    for n = width - 1, 0, -1 do
        local byte = byteAt(address + n)
        if byte == nil then return nil end
        value = value * 256 + byte
    end
    return value
end

-- An address in RAM as it is shown everywhere in a run's files: in the 80000000 range, whichever
-- mirror the code used.
local function shown(address)
    local physical = bit.band(address, 0x1fffffff)
    if physical < 0x800000 then return hex(0x80000000 + bit.band(physical, 0x1fffff)) end
    return hex(address)
end

local function registerList(numbers)
    local regs = PCSX.getRegisters().GPR.r
    local parts = {}
    for _, n in ipairs(numbers) do parts[#parts + 1] = NAMES[n] .. '=' .. hex(regs[n]) end
    return table.concat(parts, ' ')
end

local ALL = {}
for n = 1, 31 do ALL[n] = n end

local function logFile(name)
    if not logs[name] then logs[name] = assert(io.open(plan.prefix .. '-' .. name .. '.txt', 'wb')) end
    return logs[name]
end

-- One line for an access to watched memory. The breakpoint fires before the instruction runs, so
-- the PC is the instruction's own, and for a store the register it stores still holds the value.
local function access(kind, address, width)
    local pc = PCSX.getRegisters().pc
    local code = valueAt(pc, 4)
    local value
    if kind == 'Write' and code then
        local stored = PCSX.getRegisters().GPR.r[bit.band(bit.rshift(code, 16), 31)]
        value = width == 4 and stored or bit.band(stored, width == 2 and 0xffff or 0xff)
    else
        value = valueAt(address, width)
    end
    local digits = width * 2
    local line = string.format(
        'frame=%d pc=%s op=%s address=%s width=%d value=%s %s\n',
        now,
        hex(pc),
        code and hex(code) or '????????',
        shown(address),
        width,
        value and bit.tohex(value, digits) or string.rep('?', digits),
        registerList(SHORT)
    )
    logFile(kind == 'Write' and 'writes' or 'reads'):write(line)
end

local function arm()
    for _, watch in ipairs(plan.watches) do
        local kind = watch.kind
        breakpoints[#breakpoints + 1] = PCSX.addBreakpoint(watch.start, kind, watch.length, 'ps1probe',
            function(address, width)
                if not finished and now then access(kind, address, width) end
                return true
            end)
    end
    for _, address in ipairs(plan.exec) do
        breakpoints[#breakpoints + 1] = PCSX.addBreakpoint(address, 'Exec', 4, 'ps1probe', function()
            if not finished and now then
                local regs = PCSX.getRegisters()
                logFile('exec'):write(string.format('frame=%d pc=%s %s hi=%s lo=%s\n', now, hex(regs.pc),
                    registerList(ALL), hex(regs.GPR.n.hi), hex(regs.GPR.n.lo)))
            end
            return true
        end)
    end
end

local function dump(what)
    local path = plan.prefix .. '-' .. what.name .. '-' .. now .. '.bin'
    if what.kind == 'ram' then
        write(path, ffi.string(memory, 0x200000))
    elseif what.kind == 'scratch' then
        write(path, ffi.string(scratch, 0x400))
    elseif what.kind == 'vram' then
        local file = Support.File.open(path, 'TRUNCATE')
        file:writeMoveSlice(PCSX.GPU.getVRAM())
        file:close()
    else
        write(path, ffi.string(memory + bit.band(what.start, 0x1fffff), what.length))
    end
end

local function screenshot()
    local shot = PCSX.GPU.takeScreenShot()
    -- Python turns this into a PNG; the name carries what it needs to read the pixels.
    local path = string.format('%s/shot-%d-%dx%d-%d.raw', plan.work, now, shot.width, shot.height,
        tonumber(shot.bpp))
    local file = Support.File.open(path, 'TRUNCATE')
    file:writeMoveSlice(shot.data)
    file:close()
end

local function atFrame()
    for _, change in ipairs(plan.input[now] or {}) do
        if change[2] then pad.setOverride(change[1]) else pad.clearOverride(change[1]) end
    end
    if now == plan.trace_from then arm() end
    for _, what in ipairs(plan.dumps) do
        if what.frame == now then dump(what) end
    end
    if plan.shots[now] then screenshot() end
    if plan.registers[now] then
        local regs = PCSX.getRegisters()
        write(string.format('%s-registers-%d.txt', plan.prefix, now), string.format('pc=%s %s hi=%s lo=%s\n',
            hex(regs.pc), registerList(ALL), hex(regs.GPR.n.hi), hex(regs.GPR.n.lo)))
    end
    if plan.save and plan.save.frame == now then
        local file = Support.File.open(plan.save.path, 'TRUNCATE')
        file:writeMoveSlice(PCSX.createSaveState())
        file:close()
    end
    if now >= plan.frames then finish(0) end
end

local function start()
    now = 0
    -- Let go of everything: a state may have been saved with buttons held.
    for button = 0, 15 do pad.clearOverride(button) end
    atFrame()
end

-- As fast as the machine allows, unless asked otherwise, and silent: a speed of 0 lifts the limit.
if not plan.real_time then PCSX.settings.spu.Speed = 0 end
PCSX.settings.spu.Mute = true

PS1ProbeVsync = PCSX.Events.createEventListener('GPU::Vsync', guarded(function()
    if now == nil then
        if not plan.state then
            start()
        elseif not loading then
            -- From inside the frame the load waits until the frame is over; the event below says when.
            loading = true
            if not PCSX.loadSaveState(Support.File.open(plan.state)) then
                finish(3, 'the emulator refused the saved state ' .. plan.state)
            end
        end
        return
    end
    now = now + 1
    atFrame()
end))

PS1ProbeLoaded = PCSX.Events.createEventListener('ExecutionFlow::SaveStateLoaded', guarded(function()
    if loading and now == nil then start() end
end))
