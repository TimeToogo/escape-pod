.section .text.escapepod_restore::restore::restore,"ax",@progbits
	.globl	escapepod_restore::restore::restore
	.p2align	2
	.type	escapepod_restore::restore::restore,@function
escapepod_restore::restore::restore:

	.cfi_startproc
	sub sp, sp, #16
	.cfi_def_cfa_offset 16
	mov x9, x0

	ldr x10, [x0]

	str x0, [sp, #8]

	cbz x10, .LBB4_5

	ldr x8, [x9, #8]

	add x11, x8, #8
	b .LBB4_3

.LBB4_2:
	add x11, x11, #16

	subs x10, x10, #1

	b.eq .LBB4_5

.LBB4_3:
	ldp x0, x1, [x11, #-8]

	mov w8, #215
	//APP
	svc #0
	//NO_APP

	cbz x0, .LBB4_2

	mov w8, #172
	//APP
	svc #0
	//NO_APP

	mov w8, #129
	mov w1, #6

	//APP
	svc #0
	//NO_APP

	b .LBB4_2

.LBB4_5:
	ldr x10, [x9, #16]

	cbz x10, .LBB4_10

	ldr x8, [x9, #24]

	add x11, x8, #32
	b .LBB4_8

.LBB4_7:
	add x11, x11, #40

	subs x10, x10, #1

	b.eq .LBB4_10

.LBB4_8:
	ldp x0, x1, [x11, #-32]

	mov w8, #222

	ldur x5, [x11, #-16]

	ldpsw x2, x3, [x11, #-8]

	ldrsw x4, [x11]

	//APP
	svc #0
	//NO_APP

	cbz x0, .LBB4_7

	mov w8, #172
	//APP
	svc #0
	//NO_APP

	mov w8, #129
	mov w1, #6

	//APP
	svc #0
	//NO_APP

	b .LBB4_7

.LBB4_10:
	ldrsw x0, [x9, #48]

	add x1, sp, #8
	mov w8, #64
	mov w2, #1
	//APP
	svc #0
	//NO_APP

	cbz x0, .LBB4_12

	mov w8, #172
	//APP
	svc #0
	//NO_APP

	mov w8, #129
	mov w1, #6
	//APP
	svc #0
	//NO_APP

	ldr x8, [sp, #8]

	mov w1, #19

	ldrsw x0, [x8, #52]

	mov w8, #129

	//APP
	svc #0
	//NO_APP

	add sp, sp, #16
	.cfi_def_cfa_offset 0
	ret

