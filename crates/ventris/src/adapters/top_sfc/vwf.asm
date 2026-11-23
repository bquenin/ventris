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

buffer      = $7969         ; tile data: 32 bytes (two tiles) per text column
tilemap_top = $5980         ; copy of the tilemap: upper tile of each column
tilemap_bot = $59C0         ; and the lower one, one tilemap row down
upload_mark = $13CA         ; where the game starts copying the tile buffer to the screen

org $C8DDFB
    jsl draw

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
