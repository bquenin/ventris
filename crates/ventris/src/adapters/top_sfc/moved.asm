; Relocated compressed blocks for Tales of Phantasia (Super Famicom).
;
; Everything the game decompresses goes through one routine, at $C7:F4CF, called with the block's
; bank in A (8 bits) and its address in Y. This sits in front of it: when the block asked for is one
; that the builder has moved, the new bank and address are used instead. A block can then grow and
; move without anyone finding the places that point at it.
;
; MOVED is a list of six-byte entries, ended by a zero byte:
;       old bank, old address (2 bytes), new bank, new address (2 bytes)
;
; Scratch: $7D, which the game's routine sets to the bank itself two instructions later.

decompress_resume = $C7F4D4

org $C7F4CF
    jml moved_hook
    nop                     ; the five bytes replaced here are PHP, SEP #$20 and STZ $7B

org MOVED_CODE
moved_hook:
    php
    sep #$20
    rep #$10
    sta $7D                 ; the bank asked for
    phx
    ldx #$0000
.entry:
    lda.l MOVED,x
    beq .done               ; end of the list
    cmp $7D
    bne .next
    rep #$20
    tya
    cmp.l MOVED+1,x
    sep #$20
    bne .next
    rep #$20
    lda.l MOVED+4,x
    tay                     ; the new address
    sep #$20
    lda.l MOVED+3,x
    sta $7D                 ; and the new bank
    bra .done
.next:
    inx
    inx
    inx
    inx
    inx
    inx
    bra .entry
.done:
    lda $7D
    plx
    stz $7B
    jml decompress_resume
