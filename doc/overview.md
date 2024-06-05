# Overview
- [Introduction](#introduction)
- [Lexical elements](#lexical-elements)
- [Packages](#packages)
- [Modules](#modules)
- [Attributes](#attributes)
- [Command line tool](#command-line-tool)

## Introduction
This is a basic tutorial and overview of the **Rock** programming language.  
It assumes a basic knowledge of common programming concepts.

## Lexical elements

### Comments
Line comments begin with `//`.  
Multi-line block comments begin with `/*` and end with `*/`.
```rs
// line comment
/* block comment */
/*
    /* block comments can be nested */
*/
```

### Numbers
Numeric literals are untyped.  
Their type is determined during typechecking.
```rs
2,   1024         // integer
3.0, 23.45        // floating point
```

### Built-in constants
Keywords are reserved for commonly used constants.  
`null` is not like `nil` or `undefined` in other languages and requires an explicit cast.  
`null` is mostly used for `C` interop and working with `void*` equivalent `rawptr` type.
```c#
null              // null raw pointer
true, false       // boolean constants
```

### Characters
Character literals represent 32-bit Unicode code points:
```go
`r`, `2`, `\n`, `🔥`
```

### Strings
String literals are UTF-8 encoded byte sequences.  
Rock provides a flexible way to define string literals:
```go
"this is a string \n"      // parse escape sequences
`this is a raw string \n`  // don't parse escape sequences
c"this is a C string"      // include null terminator `\0`
```

Multi-line strings are represented by consecutive string literals:
```go
"1. first line"            // new line `\n` will be
`2. second line`           // inserted after each line,
"3. third line"            // but not after the last one.
```

These modifiers can be used together.  
For example, we can define a raw C string:
```go
c`C:\\Very\\Scary\\WindowsPath.txt`
```

### Escape sequences
- \t  - tab
- \n  - newline
- \r  - carriage return
- \0  - null terminator
- \\'   - single quote
- \\"   - double quote
- \\\\  - backslash

## Packages
Rock packages consist of `src` directory with `.rock` files  
and `Rock.toml` manifest, which contains project configuration.

To create a new package use `rock` compiler binary from your terminal.  
Full specification of the `rock` command line tool will be covered in a [later chapter](#command-line-tool).
```rs
rock new my_package        // create `my_package` in current directory
cd my_package              // cd into created package directory
rock run                   // build an run executable package
```

## Modules
Modules are represented by a single `.rock` file.  
Executable binary packages are required to have `src/main.rock` file.  
Modules contain items with private visibility, which can be changed with `pub` keyword.

### Procedures
Procedures are used to perform computation at runtime,  
in some other languages they are called *functions* or *methods*.

Procedures are defined with the `proc` keyword:
```rs
proc main() -> s32 {
    return 0;
}
```
Input parameters are defined like this:
```rs
proc int_sum(x: s32, y: s32) -> s32 {
    return x + y;
}
```
Procedures that return `void` can omit the return type:
```rs
proc do_nothing() {
    return;
}
```

### Structs
Structs are record types in Rock.  
They represent a named collection of **fields**.  

Structs are defined with the `struct` keyword:
```rs
struct Vector2 {
    x: f32;
    y: f32;
}
```

### Enums
Enums represent a set of integer constants.  
Each named enum field is called a **variant**.  
Variants can be assigned with an explicit value or auto incremented.  

Enums are defined with the `enum` keyword:
```rs
enum TileKind {
    Rock;  // 0
    Grass; // 1
    Water; // 2
    Forest = 10;
}
```

### Constants
The constant's value must be able to be evaluated at compile time.  
Constants don't have a memory address and **cannot be referenced**.

Constants are defined with the `const` keyword:
```rs
const CHUNK_SIZE: u32 = 32;
const ENABLE_GFX: bool = true;
const ERROR_MESSAGE: []u8 = "out of bounds";
```

### Globals
The global's value must be able to be evaluated at compile time.  
Globals have a memory address and **can be referenced**.  
It's value can be changed at runtime when declared with `mut` keyword.

Globals are defined with the `global` keyword:
```rs
global mut COUNTER: u64 = 0;
global THE_NUMBERS: [6]s32 = [4, 8, 15, 16, 23, 42];
```

### Imports
Imports are used to bring module or item names into scope.  
Only items declared with the `pub` keyword can be imported.  

Import adds module and optional list of items into current module's scope:
```go
import core/mem;             // import `mem` from `core`
import core/io.{ printf }    // import `io` and `printf` from `core`

proc example() {
    printf(c"imports");      // use `printf` directly
    io.printf(c"complete");  // use `io` to access `printf`
}
```
Imported module and items can be renamed.  
This can be used to avoid name conflicts or make them easier to use.  
```go
// import `physics_world_2d` as `world` from `physics`
import physics/physics_world_2d as world;

proc example() {
    world.init(0.16);
    world.simulate();
    world.deinit();
}
```

## Attributes

## Command line tool

## Basic types

| Type        | C Equivalent          | Description                    |
|-------------|-----------------------|--------------------------------|
| `s8`        | `int8_t`              | 8-bit signed integer           |
| `s16`       | `int16_t`             | 16-bit signed integer          |
| `s32`       | `int32_t`             | 32-bit signed integer          |
| `s64`       | `int64_t`             | 64-bit signed integer          |
| `ssize`     | `intptr_t`            | pointer-sized signed integer   |
| `u8`        | `uint8_t`             | 8-bit unsigned integer         |
| `u16`       | `uint16_t`            | 16-bit unsigned integer        |
| `u32`       | `uint32_t`            | 32-bit unsigned integer        |
| `u64`       | `uint64_t`            | 64-bit unsigned integer        |
| `usize`     | `uintptr_t`, `size_t` | pointer-sized unsigned integer |
| `f16`       | `_Float16`            | 16-bit floating point: IEEE-754-2008 binary16 |
| `f32`       | `float`               | 32-bit floating point: IEEE-754-2008 binary32 |
| `f64`       | `double`              | 64-bit floating point: IEEE-754-2008 binary64 |
| `bool`      | `bool`                | true or false                  |
| `char`      | (none)                | 32-bit Unicode code point      |
| `rawptr`    | `void*`               | type-erased pointer            |
| `void`      | `void`                | zero-sized non-value type      |
| `never`     | (none)                | represents diverging control flow |
