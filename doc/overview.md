# Lexical elements & literals
### Numbers
```
2 4096            // integer
2.0 100.45        // float
```

### Built-in constants
```
null              // null pointer
true, false       // boolean constants
```

### Strings and characters
Raw strings dont process the escape sequences.
Multi-line strings retain the newline and omit the carriage return characters.
```
'c' '🔥'          // char
"hello world"     // string
#"multi-line      // multi-line string
string"
```
```
`home\new\dir`    // raw string
#`multi-line      // multi-line raw string
raw string`
```

### Escape sequences
```
\n  - newline
\r  - carriage return
\t  - tab
\e  - escape
\\  - backslash
\'  - single quote (in char literals)
\"  - double quote (in string literals)
```

### Comments
Comments begin with //:
```
// comment
```
Multi-line comments begin with /* and end with */.
Multi-line comments can be nested:
```
/*
    /*
    comment
    */
*/
```

# Variable declarations
### Local variables
Local variables are allocated on the stack in the current scope.
All stack-allocated variables are zero-initialized by default.
```
{
    x := 10;       // infer the type of 'x'
    y : s32;       // default initialize 'y' to 0
    z : s32 = 10;  // declare type of 'z' and set its value
}
```
Local variables within the current scope cannot be 'shadowed' by another variable of the same name:
```
{
    x := s32;
    x : f32 = 4.0; // error: variable 'x' already exists
}                  // on scope exit 'x' is deallocated
x : bool = true;   // variable with name 'x' can be declared again
```

# Type system
### Basic types
```
bool                      - 1 byte boolean
s8, s16, s32, s64, ssize  - signed integers
u8, u16, u32, u64, usize  - unsigned integers
f32, f64                  - float types
char                      - 4 byte utf8 character
rawptr                    - pointer to any type (void*)
```

### Type signatures
```
s32         - basic type
*u32        - pointer to a type
Point       - custom struct
EnumKind    - custom enum
[5]s32      - static array 
[5][10]s32  - multi-dimentional static array
```
The type signatures are read left to right:
```
*Point       - pointer to Point
[10]Point    - array of 10 Points
[5]*Point    - array of 5 pointers to Point
[5][10]bool  - array of 5 arrays of 10 bools
```

### Array types
```
array : [2]s32;           // default initialize all elements to 0
multi_array : [2][3]s32;  // default initialize all elements to 0
```
Array initialization:
```
array := [2]s32{1, 2};
array : [2]s32 = {1, 2};
multi_array := [2][3]s32{{1, 2, 3}, {1, 2, 3}};
```

### Struct type declarations
Struct fields may include constant expressions, 
which will apply when instance of that type is default initialized.
```
Point :: struct {
    x: f32;        // default x = 0.0
    y: f32 = 2.0;  // default y = 2.0
}
```
Struct initialization:
```
point : Point;                      // default initialize each field
point := Point.{x: 2.0, y: 4.0};    // infer the type
point : Point = .{x: 2.0, y: 4.0};  // define the type separately
```
Typename can often be omitted:
```
return .{x: 2.0, y: 4.0};           // return type is known
points[0] = .{x = 2.0, y: 4.0}      // array type is known
data.point = .{x: 2.0, y: 4.0};     // field type is known
print_point(.{x: 2.0, y: 4.0});     // procedure parameter type is known
```

### Enum type declarations
Enum can have specified basic type, otherwise the type is chosen by the compiler.
Each enum variant must specify its constant value.
```
GeometryKind :: enum u8 {
    Line = 2;
    Triangle = 3;
    Quad = 4;
}
```
Enum literals:
```
point : GeometryKind;               // default initialize to the first variant (Line)
point := GeometryKind.Line;         // infer the type
point : GeometryKind = .Line;       // define the type separately
```
Typename can often be omitted:
```
return .Line;                       // return type is known
geometry[0] = .Line;                // array type is known
data.geometry = .Line;              // field type is known
print_geometry(.Line);              // procedure parameter type is known
```

## Built-in expressions

### 1. cast
Casts expression into the specified type:
```
x : f64 = 2.0;
y := cast(s32, x); // cast x into s32
```

### 2. sizeof
Returns the size of the type, evaluated at compile time:
```
size := sizeof(f32);
ptr_size = sizeof(*Point);
buf_size := sizeof([1024]u8);
```
