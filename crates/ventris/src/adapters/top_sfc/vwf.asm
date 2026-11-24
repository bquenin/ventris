; Variable-width text renderer for Tales of Phantasia (Super Famicom).
;
; The game draws a character by calling its routine at $C8:E5A9 from a single place, $C8:DDFB.
; That call is pointed here instead. This routine draws any glyph, of any width, from a font that
; lives in the expanded part of the ROM, into the same tile buffer and tilemap the game already uses.
;
; In:   $00  character code, 16 bits, as returned by the game's fetch routine
;       X    address of the text window's state in bank $7E (16-bit index register)
;
; Window state used here. The game maintains all of it; only the meaning of +0E is ours:
;       +08  text line on screen, for the tilemap
;       +09  offset in the tile buffer of the column the pen is in (16 bits)
;       +0B  number of that column, 0 to 27;  +0C  palette bits
;       +0E  the game zeroes this at the start of every line. Bit 7: something has been drawn on
;            this line. Bits 0-2: the pen's pixel offset inside its column.
;
; Font, supplied by the builder:
;       WIDTHS      one byte per glyph: how many pixels the pen advances
;       FONT_BANK   first bank of the glyph data: 64 bytes per glyph, 1024 glyphs per bank.
;                   Each of the 16 rows is two 16-bit words, the glyph's pixels and then its outline,
;                   with the leftmost pixel in the top bit.
;
; Scratch: $00-$07 and $2E-$2F, the same direct-page bytes the original routine uses.
;
; The names of places are centred here too (`centre`, at the end), since the game centres them for
; characters 14 dots wide.

buffer      = $7969         ; tile data: 32 bytes (two tiles) per text column
tilemap_top = $5980         ; copy of the tilemap: upper tile of each column
tilemap_bot = $59C0         ; and the lower one, one tilemap row down
upload_mark = $13CA         ; where the game starts copying the tile buffer to the screen

org $C8DDFB
    jsl draw

; The name of a place is shown on entering it, on one line of a box at the top of the screen. The
; game centres it at $C8:DB51 to $C8:DB9E: it counts the characters and puts half of what 16 leaves
; in front as its own spaces, 14 dots each (see `PLACE_NAMES` in mod.rs). The letters drawn here
; are narrower than that, so a translated name came out left of the middle. These bytes now call
; `centre` instead, which measures the line in dots and puts the pen where it is centred.
;
; The code it replaces is only ever entered at its start, and the drawing loop goes on at $C8:DB9F.
; The bytes left over hold calls to four of the engine's routines, which end in RTS and so have to be
; called from their own bank: each takes the window state in X, as the drawing loop gives it, and
; returns with the accumulator 16 bits wide. After them is `place_next` (see below).
org $C8DB51
    jsl centre
    jmp $DB9F

place_fetch:                ; the next code of the string, into $00; $13C4 moves past it
    jsr $E4CC
    rtl
place_name:                 ; a party member's name ($02 and the member's number)
    ldy $13C4
    jsr $DF63
    rep #$20
    rtl
place_string:               ; the name of an item or a spell ($4006 and its number)
    ldy $13C4
    jsr $E040
    rep #$20
    rtl
place_number:               ; a number ($4001 and where it is read from)
    rep #$20
    jsr $DE4D
    rep #$20
    rtl

; A place name is drawn whole in one go: after each character the engine goes back to $C8:DB9F,
; which ends the line when the string is at its end. A number is drawn from its digits, one at a
; time, after its bytes in the string are read, and that test takes no account of a number still
; being drawn (bit 4 of +0F), as it does of a name (bit 0). So a number that ended the name showed
; its first digit only. With a number being drawn, this goes on to its next digit at $C8:DC26, where
; $C8:DB9F goes when the string goes on. The game comes here, from $C8:DE21, only for the name of a
; place; other dialogue goes from $C8:DE18 to $C8:DE24.
place_next:
    m8
    x16
    lda $000F,x
    bit #$10
    bne .digit
    jmp $DB9F
.digit:
    stz $002B,x             ; as $C8:DB9F does before each character
    jmp $DC26

; After drawing a character of a place name the engine zeroes the upload mark and jumps back to
; $C8:DB9F; the jump now goes to `place_next`.
org $C8DE21
    jmp place_next

org CODE
draw:
    php
    rep #$30
    phb
    phy

    lda $00
    sec
    sbc #$0005
    sta $00                 ; glyph number
    and #$03FF
    asl a
    asl a
    asl a
    asl a
    asl a
    asl a
    sta $04
    lda $00
    xba
    and #$00FF
    lsr a
    lsr a
    clc
    adc #FONT_BANK
    sta $06                 ; $04-$06 point at the glyph; $07 is cleared
    phx
    ldx $00
    lda.l WIDTHS,x
    plx
    and #$00FF
    pha                     ; the advance width stays on the stack until the pen moves

    sep #$20
    lda #$7E
    pha
    plb                     ; data bank $7E from here on
    rep #$20

    lda $000E,x
    and #$0007
    sta $02                 ; the pen's pixel offset inside its column

    ; Nothing has been drawn right of the pen yet, so those columns are wiped before use.
    lda $0009,x
    clc
    adc #buffer
    tay
    lda $000E,x
    and #$0080
    bne .started
    jsr clear_column        ; first glyph on the line: the pen's own column is new as well
.started:
    tya
    clc
    adc #$0020
    tay
    jsr clear_column
    lda $000B,x
    and #$00FF
    cmp #$001A
    bcs .tilemap            ; pen in column 26: a third column would be outside the window
    tya
    clc
    adc #$0020
    tay
    jsr clear_column
    sep #$20
    inc $07                 ; $07 = 1: the glyph may spill into a third column
    rep #$20

.tilemap:
    lda $000B,x
    and #$FF00
    asl a
    asl a
    ora #$2000
    sta $2E                 ; palette and priority bits
    lda $0008,x
    and #$00FF
    xba
    lsr a
    clc
    adc #$0044
    sta $00
    lda $000B,x
    and #$00FF
    asl a
    clc
    adc $00
    sta $00                 ; where the pen's column sits in the tilemap
    lda $0009,x
    lsr a
    lsr a
    lsr a
    lsr a
    clc
    adc #$0060
    clc
    adc $2E                 ; the first tile of the pen's column
    phx
    ldx $00
    sta tilemap_top,x
    inc a
    sta tilemap_bot,x
    inc a
    sta tilemap_top+2,x
    inc a
    sta tilemap_bot+2,x
    inc a
    ldy $06                 ; high byte of Y is the third-column flag
    cpy #$0100
    bcc .placed
    sta tilemap_top+4,x
    inc a
    sta tilemap_bot+4,x
.placed:
    plx

    ; Each of the 16 rows: the glyph's pixels into the first bitplane, its outline into the second.
    lda $0009,x
    clc
    adc #buffer
    tay
    phx
    lda #$0010
    sta $2E
.row:
    lda [$04]
    jsr blit
    inc $04
    inc $04
    iny
    lda [$04]
    jsr blit
    inc $04
    inc $04
    iny
    dec $2E
    bne .row
    plx

    ; Tell the game which part of the tile buffer to send to the screen.
    ldy upload_mark
    bpl .marked
    lda $0009,x
    cmp #$0D60
    bcc .mark
    lda #$0D60
.mark:
    sta upload_mark
.marked:

    ; Move the pen.
    pla
    clc
    adc $02
    pha
    and #$0007
    ora #$0080
    sep #$20
    sta $000E,x
    rep #$20
    pla
    lsr a
    lsr a
    lsr a
    sta $00                 ; whole columns crossed
    sep #$20
    clc
    adc $000B,x
    sta $000B,x
    rep #$20
    lda $00
    asl a
    asl a
    asl a
    asl a
    asl a
    clc
    adc $0009,x
    sta $0009,x

    ply
    plb
    plp
    rtl

; Shift the 16-pixel row in A right by the pen's offset and merge it into the columns at Y.
; A and X are 16 bits wide. Changes A and X.
blit:
    m16
    x16
    stz $00
    ldx $02
    beq .place
.shift:
    lsr a
    ror $00                 ; pixels pushed out on the right collect in $01
    dex
    bne .shift
.place:
    sep #$20
    ora $0020,y
    sta $0020,y
    xba
    ora $0000,y
    sta $0000,y
    lda $07
    beq .done
    lda $01
    ora $0040,y
    sta $0040,y
.done:
    rep #$20
    rts

; Wipe the 32 bytes of the column at Y. A is 16 bits wide. Changes A.
clear_column:
    m16
    lda #$0000
    sta $0000,y
    sta $0002,y
    sta $0004,y
    sta $0006,y
    sta $0008,y
    sta $000A,y
    sta $000C,y
    sta $000E,y
    sta $0010,y
    sta $0012,y
    sta $0014,y
    sta $0016,y
    sta $0018,y
    sta $001A,y
    sta $001C,y
    sta $001E,y
    rts

; Put the pen where the place name about to be drawn is centred on the window, to the dot: its
; width W in dots is measured as below, and the pen goes PLACE_FIELD - W halved from the window's
; left edge, or to the edge itself if the name is wider than the window. No space is drawn before
; it. `place_name_pen` in mod.rs says the same, for the build's check.
;
; A name the game leaves in Japanese is centred the same way, so it lands where the game would not
; put it in two cases: when it has an odd number of characters, and when it holds a party member's
; name, which the game counts as one character more than it draws. The game puts such a name 7 dots
; left of the middle; this puts it on the middle.
;
; W is what the renderer will advance for each character the drawing loop puts on the line, read
; the way that loop reads it: a character's width in WIDTHS; for a party member's name, the name of
; an item or a number, the characters the engine's own routine writes out for it, so that a name the
; player typed and the digits of a number are measured as they are drawn. A speed or colour code
; takes its value byte and no room. A line break, a page break or the end of the string ends the
; measure: the box has one line.
;
; Those routines change $13C4 and the window state as they do for drawing. Both are put back as they
; were, so that the drawing loop starts from the same place: $13C4, and +0F to +3A of the window
; state, which hold the flags for a name or a number being drawn (+0F), the digits (+1D to +24, and
; +2F for the first one drawn), where to go on after a name (+33) and the last code read (+38).
;
; In:   X    the window state; accumulator 8 bits, index registers 16, data bank $7E
; Out:  +09, +0B and +0E (bit 7 clear: nothing drawn on the line yet) at the pen; +32, the number of
;       the game's spaces still to draw, zero
; Scratch: $00-$06, and what the engine's routines use when they are called for drawing
centre:
    php
    rep #$30
    lda $13C4
    pha                     ; where the string starts
    stx $00
    ldy #$000F
.save:
    lda ($00),y
    pha
    iny
    iny
    cpy #$003B
    bcc .save
    pea $0000               ; the width so far, in dots

.next:
    jsl place_fetch
    lda $00
    cmp #$FFFF
    beq .stop               ; the end of the string
    cmp #$4001
    beq .number
    cmp #$4006
    beq .string
    cmp #$0005
    bcs .character
    cmp #$0002
    beq .name
    cmp #$0003
    bcc .stop               ; a line break (0) or a page break (1)
    inc $13C4               ; speed (3) or colour (4): its value
    bra .next
.stop:
    jmp .measured
.character:
    sec
    sbc #$0005
    jsr place_width
    bra .next

.name:
    jsl place_name
    bra .words
.string:
    jsl place_string
    lda $00
    beq .stop               ; an item with no name: the drawing loop ends the line there too
; The routine has written the name out at [$D7],$13C4 as glyph numbers plus $15, one word each,
; ending in $FFFF, and put where the string goes on after it at +33.
.words:
    ldy $13C4
    lda [$D7],y
    cmp #$FFFF
    beq .after
    sec
    sbc #$0015
    jsr place_width
    inc $13C4
    inc $13C4
    bra .words
.after:
    lda $0033,x
    sta $13C4
    bra .next

; The routine has put the number's eight digits at +1D to +24 as the codes of the game's digits,
; and at +2F how many of them, from the left, are zeros that are not drawn. All the others are
; drawn, wherever the number stands in the name (see `place_next`).
.number:
    jsl place_number
    txa
    clc
    adc #$001D
    sta $04                 ; the digits
    lda $002F,x
    and #$00FF
    tay                     ; the first one drawn
.digit:
    cpy #$0008
    bcc .one
    jmp .next
.one:
    lda ($04),y
    and #$00FF
    sec
    sbc #$0005
    jsr place_width
    iny
    bra .digit

.measured:
    pla
    sta $06                 ; the width
    stx $00
    ldy #$0039
.restore:
    pla
    sta ($00),y
    dey
    dey
    cpy #$000F
    bcs .restore
    pla
    sta $13C4

    lda #PLACE_FIELD
    sec
    sbc $06
    bcs .fits
    lda #$0000              ; wider than the window
.fits:
    lsr a                   ; the pen, in dots from the window's left edge
    pha
    lsr a
    lsr a
    lsr a
    sep #$20
    sta $000B,x             ; its column
    rep #$20
    asl a
    asl a
    asl a
    asl a
    asl a
    clc
    adc $0009,x
    sta $0009,x             ; and that column's place in the tile buffer, on the line the box has
    pla
    and #$0007
    sep #$20
    sta $000E,x             ; the dot within the column; bit 7 clear
    stz $0032,x
    plp
    rtl

; Add the advance of glyph A to the width `centre` keeps on the stack. Changes A.
place_width:
    m16
    x16
    phx
    tax
    lda.l WIDTHS,x
    plx
    and #$00FF
    clc
    adc $03,s
    sta $03,s
    rts
