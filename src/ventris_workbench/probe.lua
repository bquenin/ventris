-- A headless probe for Mesen 2: run a game with scripted input and record what it does.
--
-- It is driven by environment variables, which ventris_workbench/emulator.py sets. Run by hand it needs
-- Mesen's test runner and file access for scripts:
--
--   PROBE_OUT=/tmp/run PROBE_FRAMES=600 PROBE_SHOTS="300 600" ./Mesen --testRunner probe.lua game.sfc \
--       --enableStdout --doNotSaveSettings --timeout=120 --debug.scriptWindow.allowIoOsAccess=true
--
-- PROBE_OUT            path prefix for everything written
-- PROBE_FRAMES         frame at which to stop
-- PROBE_INPUT          input program: steps "FRAME:KIND:BUTTONS" separated by spaces. KIND is
--                      mash (press the buttons in turn, over and over), press (once, for a few frames),
--                      hold (keep down) or none. A step lasts until the next one starts.
-- PROBE_TRIGGER        "ADDRESS@FRAME": when the CPU executes ADDRESS at or after FRAME, the clock
--                      restarts at zero. Every frame number below then counts from that moment.
-- PROBE_INPUT_AFTER    input program used once the trigger has fired
-- PROBE_SHOTS          frames at which to save a screenshot, as PREFIX-shot-FRAME.png
-- PROBE_DUMP           "MEMORY@FRAME" items: save that memory, as PREFIX-MEMORY-FRAME.bin
-- PROBE_REGISTERS      frames at which to save every register the emulator exposes, as PREFIX-registers-FRAME.txt
-- PROBE_READS          "MEMORY:START-END" items: record which code reads that range, in PREFIX-reads.txt
-- PROBE_WRITES         the same for writes, in PREFIX-writes.txt
-- PROBE_EXEC           "START-END": record every instruction executed in that range of CPU addresses,
--                      with the processor flags, in PREFIX-exec.txt
-- PROBE_LOG            "ADDRESS" or "ADDRESS:MEMORY:START-END" items: every time the CPU executes ADDRESS,
--                      add a line to PREFIX-log.txt with the frame, the registers, the top of the stack
--                      (where the routine was called from) and the bytes of that memory range
-- PROBE_TRACE_FROM     frame from which reads, executed instructions and log lines are recorded
-- PROBE_STATE          saved state to start from
-- PROBE_SAVE           "FILE@FRAME": save a state at that frame
--
-- Addresses are hexadecimal. The exit code is 0 on success and 4 if the trigger never fired.

local function env(name, default)
  local value = os.getenv(name)
  if value == nil or value == "" then return default end
  return value
end

local out = env("PROBE_OUT", "probe")
local cpu = emu.cpuType[env("PROBE_CPU", "snes")]
local cpu_memory = emu.memType[env("PROBE_CPU_MEMORY", "snesMemory")]
local last_frame = tonumber(env("PROBE_FRAMES", "600"))
local trace_from = tonumber(env("PROBE_TRACE_FROM", "0"))
local trigger_timeout = tonumber(env("PROBE_TRIGGER_TIMEOUT", "30000"))

local function parse_program(text)
  local steps = {}
  for token in string.gmatch(text, "%S+") do
    local frame, kind, buttons = string.match(token, "^(%d+):(%a+):?([%a,]*)$")
    if not frame then error("bad input step: " .. token) end
    local list = {}
    for button in string.gmatch(buttons, "[^,]+") do list[#list + 1] = button end
    steps[#steps + 1] = {frame = tonumber(frame), kind = kind, buttons = list}
  end
  table.sort(steps, function(a, b) return a.frame < b.frame end)
  return steps
end

local function parse_frames(text)
  local set = {}
  for token in string.gmatch(text, "%S+") do set[tonumber(token)] = true end
  return set
end

local program_before = parse_program(env("PROBE_INPUT", ""))
local program_after = parse_program(env("PROBE_INPUT_AFTER", ""))
local shots = parse_frames(env("PROBE_SHOTS", ""))
local register_frames = parse_frames(env("PROBE_REGISTERS", ""))

local trigger_address, trigger_frame = string.match(env("PROBE_TRIGGER", ""), "^(%x+)@(%d+)$")
if trigger_address then trigger_address, trigger_frame = tonumber(trigger_address, 16), tonumber(trigger_frame) end

local frame = 0     -- frames since power-on, or since the saved state was loaded
local fired = nil   -- frame at which the trigger fired

-- The clock the settings refer to: frames since the trigger when there is one, else since the start.
local function clock()
  if not trigger_address then return frame end
  if fired then return frame - fired end
  return -1
end

-- Input

local function buttons_for(program, time)
  local step = nil
  for _, candidate in ipairs(program) do
    if candidate.frame <= time then step = candidate end
  end
  if not step or step.kind == "none" or #step.buttons == 0 then return nil end
  local pressed = {}
  local elapsed = time - step.frame
  if step.kind == "mash" then
    local phase = elapsed % (20 * #step.buttons)
    if phase % 20 < 4 then pressed[step.buttons[phase // 20 + 1]] = true end
  elseif step.kind == "press" then
    if elapsed < 6 then for _, b in ipairs(step.buttons) do pressed[b] = true end end
  elseif step.kind == "hold" then
    for _, b in ipairs(step.buttons) do pressed[b] = true end
  else
    error("unknown input kind: " .. step.kind)
  end
  return pressed
end

emu.addEventCallback(function()
  local pressed
  if trigger_address and not fired then
    pressed = buttons_for(program_before, frame)
  elseif trigger_address then
    pressed = buttons_for(program_after, frame - fired)
  else
    pressed = buttons_for(program_before, frame)
  end
  if pressed and next(pressed) then emu.setInput(pressed, 0) end
end, emu.eventType.inputPolled)

-- Saved states can only be loaded or made from inside a callback on an executed instruction, so one is
-- attached for exactly as long as it is needed.

local pending = 0  -- actions still waiting for an instruction to run

local function once_on_any_instruction(action)
  local reference
  pending = pending + 1
  reference = emu.addMemoryCallback(function()
    emu.removeMemoryCallback(reference, emu.callbackType.exec, 0x000000, 0xFFFFFF, cpu, cpu_memory)
    action()
    pending = pending - 1
  end, emu.callbackType.exec, 0x000000, 0xFFFFFF, cpu, cpu_memory)
end

local state_path = env("PROBE_STATE", "")
local waiting_for_state = state_path ~= ""
if waiting_for_state then
  local file = assert(io.open(state_path, "rb"))
  local data = file:read("a")
  file:close()
  once_on_any_instruction(function()
    if not emu.loadSavestate(data) then error("could not load the saved state") end
    frame = 0
    waiting_for_state = false
  end)
end

local save_path, save_frame = string.match(env("PROBE_SAVE", ""), "^(.+)@(%d+)$")
if save_frame then save_frame = tonumber(save_frame) end

-- The trigger

if trigger_address then
  emu.addMemoryCallback(function()
    if not fired and not waiting_for_state and frame >= trigger_frame then
      fired = frame
      print("trigger fired at frame " .. frame)
    end
  end, emu.callbackType.exec, trigger_address, trigger_address, cpu, cpu_memory)
end

-- Which code reads or writes a range of memory

-- Work RAM can also be written through the console's data port: the address is set at $2181-$2183 and
-- each byte written to $2180 goes there and moves it on. A watch on the RAM itself never sees these.
local port_position = 0
local port_watchers = {}

local function follow_port()
  emu.addMemoryCallback(function(address, value)
    local register = address & 0xFFFF
    if register == 0x2181 then
      port_position = (port_position & 0x1FF00) | value
    elseif register == 0x2182 then
      port_position = (port_position & 0x100FF) | (value << 8)
    elseif register == 0x2183 then
      port_position = (port_position & 0x0FFFF) | ((value & 1) << 16)
    else
      for _, watcher in ipairs(port_watchers) do
        if port_position >= watcher.first and port_position <= watcher.last then
          watcher.record(port_position, value, " through $2180")
        end
      end
      port_position = (port_position + 1) & 0x1FFFF
    end
  end, emu.callbackType.write, 0x2180, 0x2183, cpu, emu.memType.snesRegister)
end

local function watch(setting, kind)
  local seen, order = {}, {}
  for item in string.gmatch(env(setting, ""), "%S+") do
    local memory, first, last = string.match(item, "^(%w+):(%x+)-(%x+)$")
    if not memory or not emu.memType[memory] then error("bad memory range: " .. item) end
    local function record(address, value, note)
      local time = clock()
      if time < trace_from or waiting_for_state then return end
      local state = emu.getState()
      local key = string.format("%s %02x%04x", item, state["cpu.k"], state["cpu.pc"]) .. (note or "")
      local entry = seen[key]
      if not entry then
        entry = {
          count = 0, low = address, high = address, first = time,
          registers = string.format("value=%02x a=%04x x=%04x y=%04x dbr=%02x d=%04x", value, state["cpu.a"], state["cpu.x"], state["cpu.y"], state["cpu.dbr"], state["cpu.d"]),
        }
        seen[key] = entry
        order[#order + 1] = key
      end
      entry.count = entry.count + 1
      entry.last = time
      entry.last_value = value
      if address < entry.low then entry.low = address end
      if address > entry.high then entry.high = address end
    end
    emu.addMemoryCallback(record, kind, tonumber(first, 16), tonumber(last, 16), cpu, emu.memType[memory])
    if kind == emu.callbackType.write and memory == "snesWorkRam" then
      if #port_watchers == 0 then follow_port() end
      port_watchers[#port_watchers + 1] = {first = tonumber(first, 16), last = tonumber(last, 16), record = record}
    end
  end
  return {seen = seen, order = order, active = env(setting, "") ~= ""}
end

local reads = watch("PROBE_READS", emu.callbackType.read)
local writes = watch("PROBE_WRITES", emu.callbackType.write)

-- Which instructions run in a range of code

local executed, executed_count, executed_order = {}, {}, {}
local exec_first, exec_last = string.match(env("PROBE_EXEC", ""), "^(%x+)-(%x+)$")
if exec_first then
  emu.addMemoryCallback(function(address)
    if clock() < trace_from or waiting_for_state then return end
    local count = executed_count[address]
    if count then
      executed_count[address] = count + 1
      return
    end
    executed_count[address] = 1
    local state = emu.getState()
    executed[address] = string.format("%02x a=%04x x=%04x y=%04x dbr=%02x", state["cpu.ps"], state["cpu.a"], state["cpu.x"], state["cpu.y"], state["cpu.dbr"])
    executed_order[#executed_order + 1] = address
  end, emu.callbackType.exec, tonumber(exec_first, 16), tonumber(exec_last, 16), cpu, cpu_memory)
end

-- A log line every time one instruction runs

local log_lines = {}
local log_limit = tonumber(env("PROBE_LOG_LIMIT", "20000"))
local log_active = env("PROBE_LOG", "") ~= ""
for item in string.gmatch(env("PROBE_LOG", ""), "%S+") do
  local address, memory, first, last = string.match(item, "^(%x+):(%w+):(%x+)-(%x+)$")
  if not address then address = string.match(item, "^(%x+)$") end
  if not address or (memory and not emu.memType[memory]) then error("bad log item: " .. item) end
  if memory then first, last = tonumber(first, 16), tonumber(last, 16) end
  emu.addMemoryCallback(function()
    local time = clock()
    if time < trace_from or waiting_for_state or #log_lines >= log_limit then return end
    local state = emu.getState()
    local stack = {}
    for offset = 1, 12 do
      stack[#stack + 1] = string.format("%02x", emu.read((state["cpu.sp"] + offset) & 0xFFFF, cpu_memory))
    end
    local line = string.format("%s frame=%d a=%04x x=%04x y=%04x dbr=%02x d=%04x p=%02x stack=%s", address, time,
      state["cpu.a"], state["cpu.x"], state["cpu.y"], state["cpu.dbr"], state["cpu.d"], state["cpu.ps"], table.concat(stack, " "))
    if memory then
      local bytes = {}
      for position = first, last do bytes[#bytes + 1] = string.format("%02x", emu.read(position, emu.memType[memory])) end
      line = line .. string.format(" %s:%x=%s", memory, first, table.concat(bytes, " "))
    end
    log_lines[#log_lines + 1] = line
  end, emu.callbackType.exec, tonumber(address, 16), tonumber(address, 16), cpu, cpu_memory)
end

-- Memory dumps

local dumps = {}
for item in string.gmatch(env("PROBE_DUMP", ""), "%S+") do
  local memory, at = string.match(item, "^(%w+)@(%d+)$")
  if not memory or not emu.memType[memory] then error("bad dump: " .. item) end
  dumps[#dumps + 1] = {memory = memory, frame = tonumber(at)}
end

local function write_file(path, data)
  local file = assert(io.open(path, "wb"))
  file:write(data)
  file:close()
end

local function dump_memory(memory, time)
  local kind = emu.memType[memory]
  local bytes = {}
  for address = 0, emu.getMemorySize(kind) - 1 do bytes[#bytes + 1] = string.char(emu.read(address, kind)) end
  write_file(string.format("%s-%s-%d.bin", out, memory, time), table.concat(bytes))
end

local function dump_registers(time)
  local lines = {}
  for key, value in pairs(emu.getState()) do lines[#lines + 1] = key .. "=" .. tostring(value) end
  table.sort(lines)
  write_file(string.format("%s-registers-%d.txt", out, time), table.concat(lines, "\n") .. "\n")
end

local function write_accesses(watcher, name)
  if not watcher.active then return end
  local lines = {"# range, address the CPU was at (usually just past the instruction), accesses, span, frames, value and registers at the first, last value"}
  for _, key in ipairs(watcher.order) do
    local entry = watcher.seen[key]
    lines[#lines + 1] = string.format("%s n=%d span=%06x-%06x frames=%d-%d %s last=%02x", key, entry.count, entry.low, entry.high, entry.first, entry.last, entry.registers, entry.last_value)
  end
  write_file(out .. "-" .. name .. ".txt", table.concat(lines, "\n") .. "\n")
end

local function write_reports()
  write_accesses(reads, "reads")
  write_accesses(writes, "writes")
  if log_active then write_file(out .. "-log.txt", table.concat(log_lines, "\n") .. "\n") end
  if exec_first then
    table.sort(executed_order)
    local lines = {}
    for _, address in ipairs(executed_order) do
      lines[#lines + 1] = string.format("%06x %s n=%d", address, executed[address], executed_count[address])
    end
    write_file(out .. "-exec.txt", table.concat(lines, "\n") .. "\n")
  end
end

emu.addEventCallback(function()
  if waiting_for_state then return end
  frame = frame + 1
  local time = clock()
  if time >= 0 then
    if shots[time] then
      write_file(string.format("%s-shot-%d.png", out, time), emu.takeScreenshot())
    end
    if register_frames[time] then dump_registers(time) end
    for _, dump in ipairs(dumps) do
      if dump.frame == time then dump_memory(dump.memory, time) end
    end
    if save_frame == time then
      once_on_any_instruction(function() write_file(save_path, emu.createSavestate()) end)
    end
    if time >= last_frame and pending == 0 then
      write_reports()
      emu.stop(0)
    end
  elseif frame >= trigger_timeout then
    write_reports()
    print("the trigger never fired")
    emu.stop(4)
  end
end, emu.eventType.endFrame)
