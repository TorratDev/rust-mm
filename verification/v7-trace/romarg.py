"""Lets a probe leave out its ROM argument (#643): if RESCUE_ROM is set and the first argument
is not a .nds file, RESCUE_ROM is put in front of the arguments. Import it, then call fill()
before the probe parses sys.argv. A ROM given on the command line is always used as given."""
import os, sys


def fill():
    rom = os.environ.get("RESCUE_ROM")
    if not rom or len(sys.argv) < 2:
        return
    first = sys.argv[1]
    if first.endswith(".nds") or first in ("-h", "--help"):
        return
    sys.argv[1:1] = [rom]
