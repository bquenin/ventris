; G.O.D pure: the renderer that draws each character at its own width (see vwf.rs).
;
; The game keeps the text of a window as a grid of codes, a 16-bit code to a cell, which its writers
; fill a character at a time ($80024CE8 types one a frame, $80024F70 writes a string at once) and
; its draw routines go through every frame, a textured quad for each code: $80025BFC for the
; dialogue's font, $80025E30 for it while the window opens, $800261A4 for the small font of the
; battle's window. A cell was 14 pixels wide (12 in the small font), and a line was as many cells as
; the window is wide. Here a line has four times as many cells, a code moves the pen by its own
; width (`widths`), and a line ends when the pen reaches the window's width. A code of the game's
; own font, and an empty cell, are as wide as they were, so the game's text is drawn as before.
;
; The menus' screens draw a string of the dialogue's encoding in the small font without a grid
; ($80058828); there the pen moves by the code's width too.
;
; Runs of one byte a character. The renderer's characters are written a byte each: the byte RUN
; ($0A, the high half of codes past the font's, which no string of the game writes) starts a run,
; in which each byte from $10 up is a character of the renderer's, its code in `run_codes`; RUN
; again, or any other byte under $10 (a line break, a wait, a tag), ends it. Each window keeps
; whether it is in a run (`runs`), set back when the window is given a new string; the menus'
; screens keep one (`menu_run`), set back when they start a string.
;
; Given: CODE, where this code goes. Register conventions of the routines below are in their notes;
; the game's registers that are live at each hook are kept.

POOL = $80110830        ; the records of the four windows the game opens, $28 bytes each
POOL_END = $801108D0
COLUMNS = 4             ; record: the window's width in cells
FLAGS = $26             ; record: $0800 for a window of the small font
COLUMN = $0C            ; record: where the pen is, in cells
ROW = $0E
GRID = $20              ; record: the grid of codes
SMALL_FONT = $0800
LARGE = 14              ; the width of a glyph of the game's own, in each font
SMALL = 12
TABLE_CODES = $0B00     ; the codes `widths` has: every code a string can write
RUN = $0A               ; the byte that starts or ends a run of the renderer's characters
RUN4 = $28              ; RUN times 4, as the writers have it to find its routine
OWN_CODES = $80091328   ; the game's table of the code of each byte

        org CODE

; stride: v0 = how many cells a line of the window at a0 has: four times its width for the four
; windows the game opens, its width for any other (the one-line window at $801058C8, whose grid
; is not on the heap). Clobbers at, v1.
stride:
        lhu   v0, COLUMNS(a0)
        lui   at, %hi(POOL)
        addiu at, at, %lo(POOL)
        sltu  at, a0, at
        bnez  at, .done
        lui   v1, %hi(POOL_END)
        addiu v1, v1, %lo(POOL_END)
        sltu  at, a0, v1
        beqz  at, .done
        nop
        sll   v0, v0, 2
.done:
        jr    ra
        nop

; stride_v1, stride_a2: the same for the window at s0, into v1 or a2. Clobber at alone, for the
; hooks in the routine that starts a new line ($80024858) and the one that clears a window.
stride_v1:
        lhu   v1, COLUMNS(s0)
        lui   at, %hi(POOL)
        addiu at, at, %lo(POOL)
        sltu  at, s0, at
        bnez  at, .done
        lui   at, %hi(POOL_END)
        addiu at, at, %lo(POOL_END)
        sltu  at, s0, at
        beqz  at, .done
        nop
        sll   v1, v1, 2
.done:
        jr    ra
        nop

stride_a2:
        lhu   a2, COLUMNS(s0)
        lui   at, %hi(POOL)
        addiu at, at, %lo(POOL)
        sltu  at, s0, at
        bnez  at, .done
        lui   at, %hi(POOL_END)
        addiu at, at, %lo(POOL_END)
        sltu  at, s0, at
        beqz  at, .done
        nop
        sll   a2, a2, 2
.done:
        jr    ra
        nop

; width_large, width_small: v1 = the width of code v1 in the dialogue's font or the small one.
; Clobbers at.
width_large:
        sltiu at, v1, TABLE_CODES
        beqz  at, .default
        lui   at, %hi(widths)
        addu  at, at, v1
        lbu   v1, %lo(widths)(at)
        jr    ra
        andi  v1, v1, $F
.default:
        jr    ra
        li    v1, LARGE

width_small:
        sltiu at, v1, TABLE_CODES
        beqz  at, .default
        lui   at, %hi(widths)
        addu  at, at, v1
        lbu   v1, %lo(widths)(at)
        jr    ra
        srl   v1, v1, 4
.default:
        jr    ra
        li    v1, SMALL

; pen: v0 = where the pen of the window at a0 is, in pixels from the line's start, at cell a1 of
; its line: the widths of the cells before it, an empty one as wide as a glyph of the game's.
; Clobbers at, v1, a1, a2, a3, t8; keeps a0.
pen:
        move  t8, ra
        jal   stride
        nop
        lhu   v1, ROW(a0)
        lw    a2, GRID(a0)
        mult  v1, v0
        lhu   a3, FLAGS(a0)
        mflo  v1
        sll   v1, v1, 1
        addu  a2, a2, v1
        andi  a3, a3, SMALL_FONT
        beqz  a1, .done
        move  v0, zero
.next:
        lhu   v1, 0(a2)
        beqz  a3, .large
        addiu a2, a2, 2
        jal   width_small
        nop
        b     .add
        nop
.large:
        jal   width_large
        nop
.add:
        addu  v0, v0, v1
        addiu a1, a1, -1
        bnez  a1, .next
        nop
.done:
        jr    t8
        nop

; room: v0 = 1 while the line the pen of the window at s0 is on has room: the pen short of the
; window's width (14 or 12 pixels a cell of it), and a cell of the line left. Clobbers at, v1,
; a0 to a3, t8, t9.
room:
        move  t9, ra
        jal   stride
        move  a0, s0
        lhu   a1, COLUMN(s0)
        nop
        sltu  at, a1, v0
        beqz  at, .full
        nop
        jal   pen
        nop
        lhu   v1, COLUMNS(s0)
        lhu   a1, FLAGS(s0)
        sll   at, v1, 1
        andi  a1, a1, SMALL_FONT
        beqz  a1, .large
        sll   v1, v1, 4
        sll   at, at, 1
.large:
        subu  v1, v1, at
        jr    t9
        sltu  v0, v0, v1
.full:
        jr    t9
        move  v0, zero

; put_cell: put code a0 in the cell of the window at s0 where its pen is, and move the pen a cell.
; Clobbers at, v0, v1, a0 to a3.
put_cell:
        move  a3, ra
        move  a2, a0
        jal   stride
        move  a0, s0
        lhu   v1, ROW(s0)
        lhu   a1, COLUMN(s0)
        mult  v1, v0
        lw    v1, GRID(s0)
        mflo  v0
        addu  v0, v0, a1
        sll   v0, v0, 1
        addu  v0, v0, v1
        sh    a2, 0(v0)
        addiu a1, a1, 1
        jr    a3
        sh    a1, COLUMN(s0)

; put_typed: what the writer that types a character a frame does with code a0: put it, then v0 = 1
; if its line has room left, as the game's own test of the pen against the window's width gave.
put_typed:
        move  t7, ra
        jal   put_cell
        nop
        jal   room
        nop
        jr    t7
        nop

; The draw routines, at the end of each cell: the pen moved by the cell's width, and v0 = the cells
; a line has, which the routine counts its cells against.
adv_dialogue:                   ; $80025BFC: the pen in a3, the cell at t2
        move  a2, ra
        lhu   v1, 0(t2)
        jal   width_large
        nop
        addu  a3, a3, v1
        jal   stride
        move  a0, s0
        jr    a2
        nop

adv_zoom:                       ; $80025E30: the pen in t2, the cell at t5
        move  a2, ra
        lhu   v1, 0(t5)
        jal   width_large
        nop
        addu  t2, t2, v1
        jal   stride
        move  a0, s0
        jr    a2
        nop

adv_small:                      ; $800261A4: the pen in t0, the cell at t2, the window at t1
        move  a2, ra
        lhu   v1, 0(t2)
        jal   width_small
        nop
        addu  t0, t0, v1
        jal   stride
        move  a0, t1
        jr    a2
        nop

; The menus' screens ($80058828): note_code takes the place of the game's `andi s3, a2, $FFFF`,
; whose delay slot takes $100 from s3 before it runs, and keeps the width of code a2;
; advance_menu takes the place of the pen's move of 12 pixels, in s4, and keeps v0.
note_code:
        andi  s3, a2, $FFFF
        move  v1, s3
        move  a0, ra
        jal   width_small
        addiu s3, s3, -$100
        lui   at, %hi(menu_width)
        jr    a0
        sb    v1, %lo(menu_width)(at)

advance_menu:
        lui   at, %hi(menu_width)
        lbu   at, %lo(menu_width)(at)
        nop
        jr    ra
        addu  s4, s4, at

; --- runs of one byte a character ----------------------------------------------------------------

; run_flag: at = where the window whose record is at t7 keeps whether it is in a run: one byte of
; `runs` for each of the four windows the game opens, and one for any other. Clobbers v1.
run_flag:
        lui   at, %hi(POOL)
        addiu at, at, %lo(POOL)
        subu  v1, t7, at
        sltiu at, v1, POOL_END - POOL
        bnez  at, .pool
        srl   v1, v1, 3
        li    v1, 16
.pool:
        lui   at, %hi(runs)
        addu  at, at, v1
        jr    ra
        addiu at, at, %lo(runs)

; clear_run: the writers, at a byte under $10, v0 being the byte times 4: a byte other than RUN
; ends a run. Then at = $80010000 + v0, as the game's own instructions it takes the place of left
; it, for the table of the routines of those bytes. Keeps v0, a0, s1. Clobbers v1, t5 to t7.
clear_run:
        move  t6, ra
        xori  t5, v0, RUN4
        beqz  t5, .done
        move  t7, s0
        jal   run_flag
        nop
        sb    zero, 0(at)
.done:
        lui   at, $8001
        jr    t6
        addu  at, at, v0

; run_typed, run_instant: the byte RUN, in each writer's table of routines: a run starts, or ends.
run_typed:
        jal   run_flag
        move  t7, s0
        lbu   v1, 0(at)
        nop
        xori  v1, v1, 1
        j     $80024CF8
        sb    v1, 0(at)

run_instant:
        jal   run_flag
        move  t7, s0
        lbu   v1, 0(at)
        nop
        xori  v1, v1, 1
        j     $80024F84
        sb    v1, 0(at)

; byte_code: the writer that types, at a byte a0 from $10 up: a0 = its code, the renderer's in a
; run, else the game's. Clobbers v0, v1, at, t6, t7.
byte_code:
        move  t6, ra
        jal   run_flag
        move  t7, s0
        lbu   v1, 0(at)
        sll   v0, a0, 1
        beqz  v1, .own
        lui   at, %hi(run_codes)
        addu  at, at, v0
        lhu   a0, %lo(run_codes)(at)
        jr    t6
        nop
.own:
        lui   at, %hi(OWN_CODES)
        addu  at, at, v0
        lhu   a0, %lo(OWN_CODES)(at)
        jr    t6
        nop

; instant_code: the writer of a whole string, at a byte s1 from $10 up: s1 = its code, as
; byte_code, and v0 = 1 for the game's space ($20 outside a run), which it writes as an empty cell.
instant_code:
        move  t6, ra
        jal   run_flag
        move  t7, s0
        lbu   v1, 0(at)
        sll   a0, s1, 1
        bnez  v1, .run
        lui   at, %hi(run_codes)
        li    v0, $20
        beq   s1, v0, .space
        lui   at, %hi(OWN_CODES)
        addu  at, at, a0
        lhu   s1, %lo(OWN_CODES)(at)
        jr    t6
        move  v0, zero
.space:
        move  s1, zero
        jr    t6
        li    v0, 1
.run:
        addu  at, at, a0
        lhu   s1, %lo(run_codes)(at)
        jr    t6
        move  v0, zero

; set_string: the game's routine that gives the window at a0 the string at a1 ($8002492C), which
; also ends a run the window was left in.
set_string:
        move  t6, ra
        jal   run_flag
        move  t7, a0
        sb    zero, 0(at)
        move  ra, t6
        lhu   v0, $14(a0)
        nop
        sh    v0, $16(a0)
        lw    v0, $18(a0)
        nop
        sw    v0, $1C(a0)
        sh    zero, $14(a0)
        jr    ra
        sw    a1, $18(a0)

; The menus' screens: menu_start, at the start of a string, ends any run; menu_byte, at each byte,
; v0 = the byte at s2, and a byte under $10 other than RUN ends a run; menu_code, at a character,
; a2 = its code, with s2 at its last byte, after RUN if it starts with one.
menu_start:
        lui   at, %hi(menu_run)
        j     $8005CD4C
        sb    zero, %lo(menu_run)(at)

menu_byte:
        lbu   v0, 0(s2)
        nop
        sltiu at, v0, $10
        beqz  at, .done
        xori  at, v0, RUN
        beqz  at, .done
        lui   at, %hi(menu_run)
        sb    zero, %lo(menu_run)(at)
.done:
        jr    ra
        nop

menu_code:
        lbu   v0, 0(s2)
        nop
        xori  at, v0, RUN
        bnez  at, .read
        lui   at, %hi(menu_run)
        lbu   v1, %lo(menu_run)(at)
        addiu s2, s2, 1
        xori  v1, v1, 1
        sb    v1, %lo(menu_run)(at)
        lbu   v0, 0(s2)
        nop
.read:
        lbu   v1, %lo(menu_run)(at)
        sltiu a0, v0, RUN
        bnez  v1, .run
        nop
        beqz  a0, .one
        sll   v1, v0, 8
        lbu   a0, 1(s2)
        addiu s2, s2, 1
        jr    ra
        addu  a2, v1, a0
.one:
        sll   v0, v0, 1
        lui   at, %hi(OWN_CODES)
        addu  at, at, v0
        lhu   a2, %lo(OWN_CODES)(at)
        jr    ra
        nop
.run:
        sll   v0, v0, 1
        lui   at, %hi(run_codes)
        addu  at, at, v0
        lhu   a2, %lo(run_codes)(at)
        jr    ra
        nop

menu_width:
        .byte SMALL
menu_run:
        .byte 0
runs:
        .space 17
        .align 4

; The code of each byte in a run, by byte. The build writes them.
run_codes:
        .space 512

; Each code's width, the dialogue's font in the low four bits and the small font's in the high
; four, from code 0 (an empty cell) to TABLE_CODES. The build writes them.
widths:
        .space TABLE_CODES

; --- the hooks, in the game's code -------------------------------------------------------------

        org $800243B0           ; opening a window of the dialogue's font: four times the cells
        sll   s1, s1, 3
        org $800244D4           ; and of the small font
        sll   s1, s1, 3
        org $80024808           ; clearing a window ($0F $02): all its cells
        jal   stride_v1
        org $800248B0           ; a new line, when the window scrolls: the lines moved up
        jal   stride_v1
        org $800248EC           ; and the last line cleared
        jal   stride_a2
        org $80024EE0           ; the writer that types: put the code, and test the line's room
        jal   put_typed
        nop
        b     $80024F2C
        nop
        org $800250F8           ; the writer of a whole string: test the line's room first
        jal   room
        nop
        nop
        nop
        org $80025118           ; then put the code
        jal   put_cell
        move  a0, s1
        j     $80024F84
        nop
        org $80025DD0           ; the draw routines' pens
        jal   adv_dialogue
        org $8002615C
        jal   adv_zoom
        org $8002635C
        jal   adv_small
        org $8005896C           ; the menus' screens
        jal   note_code
        org $80058AB0
        jal   advance_menu
        org $8002492C           ; a window given a string: out of any run
        j     set_string
        org $80024D2C           ; the writers at a byte under $10: a run ends, but at RUN
        jal   clear_run
        nop
        org $80024FB8
        jal   clear_run
        nop
        org $800100A4           ; the byte RUN, in the two writers' tables of routines
        .word run_typed
        org $800100E4
        .word run_instant
        org $80024EC4           ; the writer that types, at a character: its code
        jal   byte_code
        nop
        nop
        nop
        org $800250C8           ; the writer of a whole string, at a character
        jal   instant_code
        nop
        bnez  v0, $800250F8
        nop
        beqz  s1, $80024F84
        nop
        j     $800250F8
        nop
        org $80058874           ; the menus' screens: a string starts
        jal   menu_start
        org $8005889C           ; at each byte
        jal   menu_byte
        org $8005891C           ; at a character
        jal   menu_code
        nop
        j     $8005896C
        nop
