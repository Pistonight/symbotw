# symbotw

This is my all-in-one research project for working with the original BOTW executable, with focus on both version 1.5.0 and 1.6.0 for Switch.

## Licensing
See the license for each package in their directories. Packages are MIT unless not allowed because of dependencies. Packages without a LICENSE file are All Rights Reserved.

## Packages

- symbotw: TODO: library generated from decomp project + some extra code for linking with both 1.5.0/1.6.0
- [uking-extract](./packages/uking-extract/): Extract DWARF information from the decomp project to import into a decompiler database such as IDA
- [uking-relocate](./packages/uking-relocate/): Simulate loading ExeFS into memory and perform relocation
