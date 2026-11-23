; Dialogue read from the cartridge for Tales of Phantasia (Super Famicom).
;
; The game unpacks an area's events at $7E:BFA0 and all of the area's dialogue right after them, and
; the two must end before the bank does: 16,480 bytes. A translation is longer than the game's own
; text, and in the areas with the most events or the most dialogue it no longer fits.
;
; The game only ever shows one string at a time. An event names the string by its number, the game
; finds where it starts from the table at the head of the dialogue block, puts that offset in
; $13C4, and the text engine reads from the 24-bit pointer at $D7 plus that offset until the string
; ends. $13C4 back at zero means no string is being shown.
;
; So for an area whose dialogue does not fit, nothing is unpacked. Its dialogue is kept on the
; cartridge as it is, and when an event asks for a string, that one string is copied to where the
; dialogue would have been. The game needs room for its longest string and no longer for all of
; them. Everything that reads the string afterwards is the game's own code, unchanged.
;
; DEMAND has eight bytes for each area, by its number:
;       +0  address of the area's dialogue on the cartridge (3 bytes); zero when the area's
;           dialogue is unpacked into memory as the game has always done
;       +6  the room kept for one string, in bytes (2 bytes)
; The dialogue on the cartridge is a table of 16-bit offsets, one per string and one more for where
; the last string ends, followed by the strings.
;
; The game also writes text of its own just past the dialogue: a character's name, an item's name.
; It finds that place from $1434, the size of the dialogue block, so $1434 is given the room kept
; for one string and that text lands past it as before.

STRING_AT = $0002           ; where in that room a string is copied; not zero, which means "none"

; The loader, after the events are unpacked: A is 16 bits wide, [$12] is the size of the events and
; [$16] the size of the dialogue.
org $C67A16
    jml demand_load
    nop                     ; the five bytes replaced are LDA [$16] and STA $1434

; The two places that find a string from its number. Each had the number in A, 16 bits wide (the
; second one doubled), added it twice to $D7, read the offset there and stored it in $13C4.
org $C40D1E
    jsl demand_find
    nop
    nop
    nop
    nop
    nop
    nop
    nop
org $C40EC8
    jsl demand_find_doubled
    nop
    nop
    nop
    nop
    nop
    nop

org DEMAND_CODE
demand_load:
    php
    rep #$30
    phx
    lda $13CD               ; the area
    asl a
    asl a
    asl a
    tax
    lda.l DEMAND,x
    ora.l DEMAND+1,x
    beq .in_memory
    lda.l DEMAND+6,x
    sta $1434
    plx
    plp
    jml $C67A27             ; past the unpacking of the dialogue
.in_memory:
    plx
    plp
    lda [$16]
    sta $1434
    jml $C67A1B             ; and on to unpack it

demand_find_doubled:
    lsr a
demand_find:
    php
    rep #$30
    phx
    phy
    asl a
    tay                     ; twice the number: the string's place in a table of offsets
    lda $13CD
    asl a
    asl a
    asl a
    tax
    lda.l DEMAND,x
    ora.l DEMAND+1,x
    bne .on_cartridge
    ; In memory, as the game has it. $12 already holds the bank of the pointer at $D7.
    tya
    clc
    adc $D7
    sta $10
    lda [$10]
    sta $13C4
    ply
    plx
    plp
    rtl

.on_cartridge:
    lda.l DEMAND,x
    sta $10
    sep #$20
    lda.l DEMAND+2,x
    sta $12                 ; $10-$12: the area's dialogue on the cartridge
    rep #$20
    iny
    iny
    lda [$10],y             ; where the next string starts, which is where this one ends
    pha
    dey
    dey
    lda [$10],y             ; where this one starts
    tay
    lda $D7
    clc
    adc #STRING_AT
    tax                     ; where it goes, in bank $7E
.copy:
    lda [$10],y             ; two bytes at a time; one byte too many is copied when the length is odd
    sta.l $7E0000,x
    inx
    inx
    iny
    iny
    tya
    cmp $01,s
    bcc .copy
    pla
    lda #STRING_AT
    sta $13C4
    ply
    plx
    plp
    rtl
