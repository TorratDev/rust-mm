@ V7 synthetic test ROM, ARM9 side. Loaded at 0x02000000.
@ Each frame: wait for VBlank, bump a counter, mix KEYINPUT into an LCG,
@ store both at 0x02100000. tick_done (0x02000044) runs once per frame.
@ Words are copied into make_test_rom.py.
.syntax unified
.arm
_start:
  ldr r4, =0x04000006
  ldr r5, =0x02100000
  mov r6, #0
  ldr r7, =0x01234567
  ldr r8, =1103515245
  ldr r9, =12345
  ldr r10, =0x04000130
wait_vblank:
  ldrh r0, [r4]
  cmp r0, #192
  bne wait_vblank
  add r6, r6, #1
  ldrh r1, [r10]
  eor r7, r7, r1
  mla r7, r8, r7, r9
  str r6, [r5]
  str r7, [r5, #4]
  strh r1, [r5, #8]
tick_done:
  mov r0, r0
wait_leave:
  ldrh r0, [r4]
  cmp r0, #192
  beq wait_leave
  b wait_vblank
.ltorg
