; Translated character names for Tales of Phantasia (Super Famicom).
;
; The game keeps each party member's name as an 8-byte record in work RAM, in the naming screen's own
; one-byte encoding, ending in $FF. When dialogue prints a name, the routine at $C8:DFC0 converts the
; record into 16-bit values (glyph number + $15, ending in $FFFF) just past the dialogue block, and the
; text engine reads those.
;
; This replaces that routine. If the record holds the default name a new game starts with, the pack's
; name for dialogue is written; anything else, such as a name the player typed, is converted byte by
; byte. The records themselves and the save data are not touched.
;
; In:   Y  address of the name record, bank $7E
;       X  address to write the converted name to, bank $7E
;
; Tables, supplied by the builder, one slot per character in the same order:
;       NAME_DEFAULTS     8 bytes: the default record, ending in $FF
;       NAME_TRANSLATED  32 bytes: the pack's name, already converted; a first word of zero means the
;                         pack does not rename this character
;
; And one table for everything else that is converted this way, such as an item name in dialogue:
;       NAME_GLYPHS      256 words: for each byte of the menu encoding, its glyph number plus $15


org $C8DF9C
    jsl name_hook
org $C8E06D
    jsl name_hook

org NAME_CODE
name_hook:
    php
    rep #$30
    phb
    phy
    phx
    sep #$20
    lda #$7E
    pha
    plb
    rep #$20

    ldx #$0000              ; offset of the default being compared
.try:
    lda $03,s
    tay                     ; Y = the record
.byte:
    sep #$20
    lda.l NAME_DEFAULTS,x
    cmp $0000,y
    rep #$20
    bne .differs
    and #$00FF
    cmp #$00FF
    beq .found              ; matched up to and including the terminator
    inx
    iny
    bra .byte
.differs:
    txa
    and #$FFF8
    clc
    adc #$0008
    tax
    cpx #NAME_DEFAULTS_SIZE
    bcc .try
.original:
    ; Not a default name, or one the pack leaves alone: convert it as the game does, but through
    ; NAME_GLYPHS, which agrees with the pack's menu font where the game's own table would not.
    lda $03,s
    tay                     ; Y = the record
    lda $01,s
    tax                     ; X = where the converted name goes
.convert:
    lda $0000,y
    and #$00FF
    cmp #$00FF
    beq .end
    iny
    cmp #$00FC
    bne .glyph
    lda #$000D              ; a line break
    bra .store
.glyph:
    phx
    asl a
    tax
    lda.l NAME_GLYPHS,x
    plx
.store:
    sta $0000,x
    inx
    inx
    bra .convert
.end:
    lda #$FFFF
    sta $0000,x
    plx
    ply
    plb
    plp
    rtl

.found:
    txa
    and #$FFF8
    asl a
    asl a
    tax                     ; 32 bytes per translated name
    lda.l NAME_TRANSLATED,x
    beq .original
    lda $01,s
    tay                     ; Y = where the converted name goes
.copy:
    lda.l NAME_TRANSLATED,x
    sta $0000,y
    inx
    inx
    iny
    iny
    cmp #$FFFF
    bne .copy
    plx
    ply
    plb
    plp
    rtl
