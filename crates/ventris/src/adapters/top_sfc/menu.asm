; Longer menu strings for Tales of Phantasia (Super Famicom).
;
; Menu text is a pool of strings at $EF:7000. The game finds a string by its offset in the pool, and
; that offset is scattered through code and tables, so a string cannot simply be moved. Instead:
;
;   - a string that outgrows its slot is written in bank MENU_BANK, at an address of $8000 or above;
;   - its old slot starts with $FD followed by that address;
;   - every routine that reads the pool fetches its bytes through the routines below, which follow
;     the $FD marker and then keep reading from the new place.
;
; An index below $8000 is an offset in the original pool; $8000 and above is an address in MENU_BANK.
; Nine routines read the pool. Eight read a string from its start; one ($C2:4957) goes straight to
; a given character, so the string is resolved before the character's position is added.

pool = $EF7000

; --- readers that index the pool with X, 8-bit accumulator
org $C2495E
    jsl menu_fetch_x
org $C24D02
    jsl menu_fetch_x
org $C2AB75
    jsl menu_fetch_x
org $C2C165
    jsl menu_fetch_x
org $C8E050
    jsl menu_fetch_x

; --- readers that index it with Y and then mask the byte to 16 bits (LDA $7000,Y / AND #$00FF)
org $C2B5E8
    jsl menu_fetch_y
    nop
    nop
org $C9D0BB
    jsl menu_fetch_y
    nop
    nop
org $C9D76D
    jsl menu_fetch_y
    nop
    nop

; --- a reader that indexes it with Y and stores the byte (LDA $7000,Y / STA [$88])
org $C9D014
    jsl menu_fetch_y_store
    nop

; --- the reader that picks one character: was TXA / CLC / ADC $02
org $C24957
    jsl menu_resolve_add

org MENU_CODE

; In: X = index. Out: A = the byte there (8 bits); X moved to the new place if the string has one.
menu_fetch_x:
    m8
    x16
    cpx #$8000
    bcs .moved
    lda.l pool,x
    cmp #$FD
    bne .done
    rep #$20
    lda.l pool+1,x
    tax
    sep #$20
.moved:
    lda.l MENU_BANK,x
.done:
    rtl

; In: Y = index. Out: A = the byte there, as 16 bits; Y moved likewise.
menu_fetch_y:
    m16
    x16
    phx
    tyx
    sep #$20
    jsl menu_fetch_x
    rep #$20
    and #$00FF
    txy
    plx
    rtl

; In: Y = index. Stores the byte through the pointer at $88 and leaves it in A (8 bits).
menu_fetch_y_store:
    m8
    x16
    phx
    tyx
    jsl menu_fetch_x
    txy
    plx
    sta [$88]
    rtl

; In: X = index of a string. Out: A = index of its character number $02.
menu_resolve_add:
    php
    sep #$20
    rep #$10
    cpx #$8000
    bcs .resolved
    lda.l pool,x
    cmp #$FD
    bne .resolved
    rep #$20
    lda.l pool+1,x
    tax
.resolved:
    plp
    txa
    clc
    adc $02
    rtl
