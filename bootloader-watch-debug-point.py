import gdb

gdb.execute("file ./build/bootloader.efi")
gdb.execute("target remote localhost:1234")
gdb.execute("watch *(int*)0x200000 = 0xdeadbeef")
gdb.execute("c")
