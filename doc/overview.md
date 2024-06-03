# Overview

## Introduction
This is a basic tutorial and overview of the **Rock** programming language.  
It assumes a basic knowledge of common programming concepts.

## Lexical elements and literals

### Comments
Line comments begin with `//`.  
Multi-line block comments begin with `/*` and end with `/*`.
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
`null` is not like `nil` or `undefined` and requires an explicit cast.  
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
