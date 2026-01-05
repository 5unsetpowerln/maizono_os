import gdb
import sys
import os

target = "./build/kernel.elf"
gdb_server = "localhost:1234"


def is_interpretable_as_int(s: str):
    try:
        int(s)
        return True
    except ValueError:
        return False


gdb.execute(f"file {target}")
gdb.execute(f"target remote {gdb_server}")
gdb.execute("dir kernel")

gdb.execute("hb _start")
gdb.execute("c")
gdb.execute("b *kernel::main")
gdb.execute("c")
gdb.execute("b *kernel::gdt::call_app")
gdb.execute("c")
# gdb.execute("b *kernel::interrupts::general_protection_exception_interrupt_handler")
gdb.execute("b kernel::interrupts::page_fault_exception_interrupt_handler")
gdb.execute("b *0x22dacf")

# gdb.execute("hb *kernel::main+535")
# gdb.execute("c")

# gdb.execute("hb *kernel::task::TaskManager::wakeup_by_key+0x9a")
# gdb.execute("c")
