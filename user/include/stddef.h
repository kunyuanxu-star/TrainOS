#ifndef _STDDEF_H
#define _STDDEF_H

#include <stdint.h>

#define NULL ((void*)0)

typedef intptr_t ptrdiff_t;
typedef uintptr_t size_t;

typedef uint16_t wchar_t;

#define offsetof(type, member) __builtin_offsetof(type, member)

typedef struct {
    long quot;
    long rem;
} div_t;

typedef struct {
    long long quot;
    long long rem;
} lldiv_t;

#endif /* _STDDEF_H */
