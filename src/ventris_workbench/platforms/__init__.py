"""What is about a console and not about one game, for taking a game apart.

``snes`` is the cartridge layout: copier headers, the internal header and its checksum, address
mapping. ``w65816`` disassembles its code. ``ps2`` finds and reads the files inside a disc image.
The engine has its own of each, in Rust, under ``crates/ventris/src/platforms``.
"""
