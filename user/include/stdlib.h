#ifndef _STDLIB_H
#define _STDLIB_H

#include <stddef.h>

#define EXIT_SUCCESS 0
#define EXIT_FAILURE 1

#define RAND_MAX 32767

typedef struct { int quot; int rem; } div_t;
typedef struct { long quot; long rem; } ldiv_t;
typedef struct { long long quot; long long rem; } lldiv_t;

double atof(const char* nptr);
int atoi(const char* nptr);
long atol(const char* nptr);
long long atoll(const char* nptr);

float strtof(const char* nptr, char** endptr);
double strtod(const char* nptr, char** endptr);
long double strtold(const char* nptr, char** endptr);
long strtol(const char* nptr, char** endptr, int base);
long long strtoll(const char* nptr, char** endptr, int base);
unsigned long strtoul(const char* nptr, char** endptr, int base);
unsigned long long strtoull(const char* nptr, char** endptr, int base);

int rand(void);
void srand(unsigned int seed);

void* aligned_alloc(size_t alignment, size_t size);
void* calloc(size_t nmemb, size_t size);
void* malloc(size_t size);
void free(void* ptr);
void* realloc(void* ptr, size_t size);
void* reallocarray(void* ptr, size_t nmemb, size_t size);

void* sbrk(intptr_t incr);

void abort(void) __attribute__((noreturn));
void exit(int status) __attribute__((noreturn));
void _Exit(int status) __attribute__((noreturn));

int atexit(void (*function)(void));
int on_exit(void (*function)(int status, void* arg), void* arg);

char* getenv(const char* name);
int setenv(const char* name, const char* value, int overwrite);
int unsetenv(const char* name);
int putenv(char* string);

int system(const char* command);

div_t div(int numer, int denom);
ldiv_t ldiv(long numer, long denom);
lldiv_t lldiv(long long numer, long long denom);

int abs(int j);
long labs(long j);
long long llabs(long long j);

void* bsearch(const void* key, const void* base, size_t nmemb, size_t size,
              int (*compar)(const void*, const void*));
void qsort(void* base, size_t nmemb, size_t size,
           int (*compar)(const void*, const void*));

long a64l(const char* s);
char* l64a(long value);

int mblen(const char* s, size_t n);
int mbtowc(wchar_t* pwc, const char* s, size_t n);
int wctomb(char* s, wchar_t wc);
size_t mbstowcs(wchar_t* pwcs, const char* s, size_t n);
size_t wcstombs(char* s, const wchar_t* pwcs, size_t n);

#endif /* _STDLIB_H */
