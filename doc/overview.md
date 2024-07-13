# Overview
- [Introduction](#introduction)
- [Lexical elements](#lexical-elements)
- [Packages](#packages)
- [Modules](#modules)
- [Items](#items)
- [Local variables](#local-variables)
- [Type system](#type-system)
- [Attributes](#attributes)
- [Rock manifest](#rock-manifest)
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
---

### Numbers
Numeric literals are untyped.  
Their type is determined during typechecking.
```rs
2,   1024         // integer
3.0, 23.45        // floating point
```
---

### Built-in constants
Keywords are reserved for commonly used constants.  
`null` does not behave like `nil` or `undefined` in other languages and requires an explicit cast.  
`null` is mostly used for `C` interop and working with `void*` equivalent `rawptr` type.
```c#
null              // null raw pointer
true, false       // boolean constants
```
---

### Characters
Character literals represent 32-bit Unicode code points:
```go
`r`, `2`, `\n`, `🔥`
```
---

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
c`C:\\Very\\Annoying\\WindowsPath.txt`
```
---

### Escape sequences
- \n    - newline
- \r    - carriage return
- \t    - tab
- \0    - null terminator
- \\'   - single quote
- \\"   - double quote
- \\\\  - backslash
- \\xNN - hexadecimal 8-bit character
- \\uNNNN - hexadecimal 16-bit Unicode character
- \\uNNNNNNNN - hexadecimal 32-bit Unicode character
---

## Packages
Rock packages consist of `src` directory and `Rock.toml` manifest.  
Directories can contain `.rock` files and nested directories.  
`Rock.toml` manifest will be covered in a [later chapter](#rock-manifest).

To create a **new package** use `rock` compiler binary from your terminal.  
Full specification of the `rock` command line tool will be covered in a [later chapter](#command-line-tool).
```rs
rock new my_package        // create `my_package` in current directory
cd my_package              // cd into created package directory
rock run                   // build and run executable package
```

## Modules
Modules are represented by a single `.rock` file.  
Executable binary packages are required to have `src/main.rock` file.  
Modules contain a list of **items**.  

## Items
Items are **private** by default.  
Their **visibility** can be changed via the `pub` keyword.

### Procedures
Procedures are used to perform computation at runtime,  
in some other languages they are called functions or methods.

Procedures are defined with the `proc` keyword:
```rs
proc main() -> s32 {
    return 0;
}
```

Input **parameters** are defined like this:
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
---

### Enums
Enums represent a set of integer constants.  
Each named enum field is called a **variant**.  
Variants can be assigned with an explicit value or auto incremented.  

Enums are defined with the `enum` keyword:
```rs
enum TileKind {
    Rock,  // 0
    Grass, // 1
    Water, // 2
    Forest = 10,
}
```
---

### Structs
Structs are record types in Rock.  
They represent a named collection of **fields**.  
Visibility rules do apply to struct fields.

Structs are defined with the `struct` keyword:
```rs
struct Vector2 {
    pub x: f32,
    pub y: f32,
}
```
---

### Constants
The constant's value must be **evaluatable at compile time**.  
Constants don't have a memory address and **cannot be referenced**.  
It's value cannot be changed at runtime.

Constants are defined with the `const` keyword:
```rs
const CHUNK_SIZE: u32 = 32;
const ENABLE_GFX: bool = true;
const ERROR_MESSAGE: []u8 = "out of bounds";
```
---

### Globals
The global's value must be **evaluatable at compile time**.  
Globals have a memory address and **can be referenced**.  
It's value can be changed at runtime, when declared with `mut` keyword.

Globals are defined with the `global` keyword:
```rs
global mut COUNTER: u64 = 0;
global CONSTANT_NUMBERS: [6]s32 = [4, 8, 15, 16, 23, 42];
```
---

### Imports
Imports are used to bring module and item names into scope.  
Only items declared with the `pub` keyword can be imported.  

Imports are defined with the `import` keyword.  
To specify the source package, use `package_name:`.  
By default, imports search in the **current package**.  
Import path is a list of `/` separated names.  
Import paths must end with the **module name**.

```go
// import `fs` from `core`
import core:fs;

// import `libc` and `printf` from `core`
import core:libc.{ printf }

proc example() {
    printf(c"directly use printf");
    libc.printf(c"access printf from libc");
}
```

Imported modules and items can be renamed.  
Underscore can be used to ignore imported modules.

```go
// import `world2D` as `world`
// from current package path:
// src/engine/physics/world2D.rock
import engine/physics/world2D as world;

// import `printf` as `log`
// from `core` and ignore `libc` by renaming it to `_`
import core:libc as _.{ printf as log }

proc example() {
    log(c"creating the world\n");
    world.init(0.16);
    log(c"simulating the world\n");
    world.simulate();
    log(c"destroying the world\n");
    world.deinit();
}
```
---

## Local Variables
Local variables can be defined in procedure scope.  
They are allocated on the stack.

Locals are defined with the `let` or `mut` keyword:
```rs
// `x` is immutable: cannot be changed
// type is inferred as `s32`
let x = 10;

// error: cannot assign to an immutable variable
x = 40;

// `y` is mutable: can be changed
// type is inferred as `s32`
mut y = 20;

// assign `50` to `y`
y = 50;
```

Rock supports type inference.  
Most of the time, type annotations aren't needed.  
To specify the type, use `:` followed by the type:

```rs
// type is specified as `u32`
let x: u32 = 10;

// type is specified as `usize`
mut y: usize = 20;
```

Rock **doesn't have shadowing**.  
Any name in scope refers to a single entity.

```rs
proc example(v: u32) -> u32 {
    let x: u32 = 10;
    let v: u32 = 20; // error: name `v` is defined multiple times

    {
        // define local `y`
        let y = x + v;
        // `y` goes out of scope
    }

    // define local `y`
    // no shadowing occurs
    let y = x * v;

    return y;
}
```

## Type system
Rock is statically and distinctly typed.  
Unlike some `C` types, Rock types can be read from left to right.

### Basic types

| Type        | C Equivalent          | Description                    |
|-------------|-----------------------|--------------------------------|
| `s8`        | `int8_t`              | 8-bit  signed integer          |
| `s16`       | `int16_t`             | 16-bit signed integer          |
| `s32`       | `int32_t`             | 32-bit signed integer          |
| `s64`       | `int64_t`             | 64-bit signed integer          |
| `ssize`     | `intptr_t`            | pointer-sized signed integer   |
| `u8`        | `uint8_t`             | 8-bit  unsigned integer        |
| `u16`       | `uint16_t`            | 16-bit unsigned integer        |
| `u32`       | `uint32_t`            | 32-bit unsigned integer        |
| `u64`       | `uint64_t`            | 64-bit unsigned integer        |
| `usize`     | `uintptr_t`, `size_t` | pointer-sized unsigned integer |
| `f32`       | `float`               | 32-bit floating point: IEEE-754-2008 binary32 |
| `f64`       | `double`              | 64-bit floating point: IEEE-754-2008 binary64 |
| `bool`      | `bool`                | true or false                  |
| `char`      | (none)                | 32-bit Unicode code point      |
| `rawptr`    | `void*`               | type-erased pointer            |
| `void`      | `void`                | zero-sized non-value type      |
| `never`     | (none)                | represents diverging control flow |

### Custom types
Custom types are `struct` and `enum` items.  
Empty struct and enums are allowed and have `0x0` size and `0x1` alignment.

```rs
// define `size` with `Vector2` struct type
let size = Vector2.{x: 2.0, y: 3.0};

// define `kind` with `TileKind` enum type
let kind = TileKind.Grass;
```
---

### Reference types
References function similarly to pointers in other languages.  
By default, references are immutable, use the `mut` keyword to make them mutable.  
Both reference types and the `address` expression use `&` or `&mut` syntax:

```rs
mut x: u32 = 10;

// take immutable reference to `x`
let immutable_ref: &u32 = &x;

// take mutable reference to `x`
let mutable_ref: &mut u32 = &mut x;
```
---

### Array static
Static arrays have a fixed length and store multiple values.  
The array **length** must **evaluatable at compile time**.  

```rs
// array of `3` values of `s32` type
let array: [3]s32 = [1, 2, 3];

// array of `2` arrays of `3` values of `s32` type
let array_2d: [2][3]s32 = [[1, 2, 3], [2, 4, 6]];

// array length can be accessed via `len` field
let array_len = array.len;
```
---

### Array slice
Slices represent a view into a sequense of elements.  
Slice consist of **pointer** and **length**, with total size of 16 bytes.

```rs
struct Data {
    values: []f32,        // immutable slice of `f32`
    values_mut: [mut]f32, // mutable slice of `f64`
}
```

Slices can be allocated or created using the `core` library  
or created using the `slice` expression (does not allocate):

```rs
let array: [3]s32 = [1, 2, 3];

// take slice of all elements
let values: []s32 = array[..];

// take mutable slice of all elements
let values_mut: [mut]s32 = array[mut ..];

// take slice from 1 up to 2 (excluding `2nd` element)
let values_2: []s32 = array[1..<2];

// take slice from 1 up to 2 (including `2nd` element)
let sub_slice: []s32 = values[1..=2];

// slice pointer can be accessed via `ptr` field
// slice length can be accessed via `len` field
let slice_ptr: &s32 = values.ptr;
let slice_len: usize = values.len;
```
---

### Procedure pointers
Procedure pointers, also known as **function pointers**,  
represent address of a procedure and can be called.  
This type of dispatch is known as **indirect call**.

```rs
proc add(x: s32, y: s32) -> s32 {
    return x + y;
}

proc sub(x: s32, y: s32) -> s32 {
    return x - y;
}

// `action` is any procedure with matching signature
// procedure pointer input parameters are not named
proc math_op(
    x: s32,
    y: s32,
    action: proc(s32, s32) -> s32,
) -> s32 {
    return action(x, y);
}

proc example() {
    let x = 10;
    let y = 20;

    // pass `add` procedure pointer
    let add_value = math_op(x, y, add);
    // add_value = 30

    // pass `sub` procedure pointer
    let sub_value = math_op(x, y, sub);
    // sub_value = -10
}
```
---

## Attributes

## Rock manifest

## Command line tool
