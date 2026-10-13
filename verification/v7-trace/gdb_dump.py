#!/usr/bin/env python3
"""Dump an ARM9 RAM range once per trigger through DeSmuME's GDB stub (V7).

Connects to the ARM9 GDB stub (desmume-cli --arm9gdb=PORT), sets a breakpoint
at the trigger address, and on each hit reads LEN bytes at ADDR and appends
them to OUT. Stops after N hits. Speaks the GDB remote serial protocol
directly, so no gdb-multiarch is needed.

Usage: python3 gdb_dump.py PORT TRIGGER ADDR LEN N OUT
       (addresses in hex, e.g. 0x02000044)
"""

import socket
import sys
import time


class Rsp:
    def __init__(self, port: int):
        for _ in range(100):
            try:
                self.s = socket.create_connection(("127.0.0.1", port))
                break
            except ConnectionRefusedError:
                time.sleep(0.1)
        else:
            sys.exit(f"no GDB stub on port {port}")
        self.buf = b""

    def _byte(self) -> bytes:
        if not self.buf:
            self.buf = self.s.recv(65536)
            if not self.buf:
                raise EOFError("stub closed the connection")
        b, self.buf = self.buf[:1], self.buf[1:]
        return b

    def cmd(self, payload: str) -> str:
        data = payload.encode()
        csum = sum(data) & 0xFF
        self.s.sendall(b"$" + data + b"#" + f"{csum:02x}".encode())
        while True:
            b = self._byte()
            if b == b"$":
                break
            # '+' acks, '-' would be a retransmit request (not seen on loopback)
        body = b""
        while (b := self._byte()) != b"#":
            body += b
        self._byte(), self._byte()  # checksum
        self.s.sendall(b"+")
        return body.decode()


def main() -> None:
    if len(sys.argv) != 7:
        sys.exit(__doc__)
    port = int(sys.argv[1])
    trigger, addr, length = (int(x, 16) for x in sys.argv[2:5])
    hits, out = int(sys.argv[5]), sys.argv[6]

    g = Rsp(port)
    g.cmd("?")
    if g.cmd(f"Z0,{trigger:x},4") != "OK":
        sys.exit("breakpoint not accepted")
    start = time.monotonic()
    with open(out, "wb") as f:
        for _ in range(hits):
            stop = g.cmd("c")
            if not stop.startswith(("S05", "T05")):
                sys.exit(f"unexpected stop reply: {stop}")
            f.write(bytes.fromhex(g.cmd(f"m{addr:x},{length:x}")))
            # Step over the breakpoint, or the next "c" stops on it again.
            g.cmd(f"z0,{trigger:x},4")
            g.cmd("s")
            g.cmd(f"Z0,{trigger:x},4")
    elapsed = time.monotonic() - start
    print(f"{hits} hits in {elapsed:.2f}s ({hits / elapsed:.1f} hits/s)")
    g.cmd(f"z0,{trigger:x},4")
    g.s.close()


if __name__ == "__main__":
    main()
