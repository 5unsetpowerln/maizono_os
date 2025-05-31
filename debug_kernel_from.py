import gdb
import sys
import os

target = "./build/kernel.elf"
gdb_server = "localhost:1234"
breakpoint_env_name = "BREAKPOINT"

def is_interpretable_as_int(s: str):
    try:
        int(s)
        return True
    except ValueError:
        return False

breakpoint = os.environ.get(breakpoint_env_name)

if breakpoint == None:
    breakpoint = ""

if not is_interpretable_as_int(breakpoint) and not breakpoint.startswith("*"):
    breakpoint = "*" + breakpoint


gdb.execute(f"file {target}")
gdb.execute(f"target remote {gdb_server}")

gdb.execute("hb _start")
gdb.execute("c")

if breakpoint != "":
    gdb.execute(f"hb {breakpoint}")
    gdb.execute("c")
