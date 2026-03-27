// Minimal stdlib implementation for TrainOS

#include <stddef.h>
#include <limits.h>

extern void* _sbrk(intptr_t incr);
extern void _exit(int code) __attribute__((noreturn));

static char* heap_end = NULL;

void* malloc(size_t size) {
    if (heap_end == NULL) {
        heap_end = (char*)_sbrk(0);
    }

    char* prev_end = heap_end;
    void* new_end = _sbrk(size);
    if (new_end == (void*)-1) {
        return NULL;
    }
    heap_end = (char*)new_end;
    return prev_end;
}

void* calloc(size_t nmemb, size_t size) {
    size_t total = nmemb * size;
    void* ptr = malloc(total);
    if (ptr) {
        unsigned char* p = (unsigned char*)ptr;
        while (total--) {
            *p++ = 0;
        }
    }
    return ptr;
}

void free(void* ptr) {
    (void)ptr;
    // Simple allocator - no actual freeing
}

void* realloc(void* ptr, size_t size) {
    if (!ptr) {
        return malloc(size);
    }
    void* new_ptr = malloc(size);
    if (new_ptr) {
        // Copy old data - simplified, assumes old size <= new size
        // In real implementation, we'd track the old size
    }
    return new_ptr;
}

void* reallocarray(void* ptr, size_t nmemb, size_t size) {
    if (nmemb != 0 && size > (size_t)-1 / nmemb) {
        return NULL;
    }
    return realloc(ptr, nmemb * size);
}

void abort(void) {
    _exit(1);
}

void exit(int status) {
    _exit(status);
}

void _Exit(int status) {
    _exit(status);
}

int atexit(void (*function)(void)) {
    (void)function;
    return 0;
}

int on_exit(void (*function)(int status, void* arg), void* arg) {
    (void)function;
    (void)arg;
    return 0;
}

int abs(int j) {
    return j < 0 ? -j : j;
}

long labs(long j) {
    return j < 0 ? -j : j;
}

long long llabs(long long j) {
    return j < 0 ? -j : j;
}

div_t div(int numer, int denom) {
    div_t result;
    result.quot = numer / denom;
    result.rem = numer % denom;
    return result;
}

ldiv_t ldiv(long numer, long denom) {
    ldiv_t result;
    result.quot = numer / denom;
    result.rem = numer % denom;
    return result;
}

lldiv_t lldiv(long long numer, long long denom) {
    lldiv_t result;
    result.quot = numer / denom;
    result.rem = numer % denom;
    return result;
}

long strtol(const char* nptr, char** endptr, int base) {
    const char* s = nptr;
    int neg = 0;
    long val = 0;

    while (*s == ' ' || *s == '\t') {
        s++;
    }

    if (*s == '-') {
        neg = 1;
        s++;
    } else if (*s == '+') {
        s++;
    }

    if ((base == 0 || base == 16) && s[0] == '0' && s[1] == 'x') {
        base = 16;
        s += 2;
    } else if (base == 0) {
        base = 10;
    }

    while (*s) {
        int digit;
        char c = *s;
        if (c >= '0' && c <= '9') {
            digit = c - '0';
        } else if (c >= 'a' && c <= 'z') {
            digit = c - 'a' + 10;
        } else if (c >= 'A' && c <= 'Z') {
            digit = c - 'A' + 10;
        } else {
            break;
        }
        if (digit >= base) {
            break;
        }
        val = val * base + digit;
        s++;
    }

    if (endptr) {
        *endptr = (char*)s;
    }
    return neg ? -val : val;
}

long long strtoll(const char* nptr, char** endptr, int base) {
    return (long long)strtol(nptr, endptr, base);
}

unsigned long strtoul(const char* nptr, char** endptr, int base) {
    return (unsigned long)strtol(nptr, endptr, base);
}

unsigned long long strtoull(const char* nptr, char** endptr, int base) {
    return (unsigned long long)strtol(nptr, endptr, base);
}

int atoi(const char* nptr) {
    return (int)strtol(nptr, NULL, 10);
}

long atol(const char* nptr) {
    return strtol(nptr, NULL, 10);
}

long long atoll(const char* nptr) {
    return strtoll(nptr, NULL, 10);
}

double strtod(const char* nptr, char** endptr) {
    // Simplified - just return 0.0
    if (endptr) {
        *endptr = (char*)nptr;
    }
    return 0.0;
}

float strtof(const char* nptr, char** endptr) {
    return (float)strtod(nptr, endptr);
}

long double strtold(const char* nptr, char** endptr) {
    return (long double)strtod(nptr, endptr);
}

double atof(const char* nptr) {
    return strtod(nptr, NULL);
}

int rand(void) {
    // Linear congruential generator - simplified
    static unsigned long next = 1;
    next = next * 1103515245 + 12345;
    return (int)(next / 65536) % RAND_MAX;
}

void srand(unsigned int seed) {
    // Would seed the RNG
}

int system(const char* command) {
    (void)command;
    return -1;
}

char* getenv(const char* name) {
    (void)name;
    return NULL;
}

int setenv(const char* name, const char* value, int overwrite) {
    (void)name;
    (void)value;
    (void)overwrite;
    return -1;
}

int unsetenv(const char* name) {
    (void)name;
    return -1;
}

int putenv(char* string) {
    (void)string;
    return -1;
}

void* bsearch(const void* key, const void* base, size_t nmemb, size_t size,
              int (*compar)(const void*, const void*)) {
    const char* b = (const char*)base;
    size_t l = 0;
    size_t r = nmemb;

    while (l < r) {
        size_t m = l + (r - l) / 2;
        int cmp = compar(key, b + m * size);
        if (cmp == 0) {
            return (void*)(b + m * size);
        } else if (cmp < 0) {
            r = m;
        } else {
            l = m + 1;
        }
    }
    return NULL;
}

void qsort(void* base, size_t nmemb, size_t size,
           int (*compar)(const void*, const void*)) {
    // Simplified quicksort
    (void)base;
    (void)nmemb;
    (void)size;
    (void)compar;
}

long a64l(const char* s) {
    (void)s;
    return 0;
}

char* l64a(long value) {
    (void)value;
    static char buf[12];
    buf[0] = '\0';
    return buf;
}

int getsubopt(char** optionp, char* const* tokens, char** valuep) {
    (void)optionp;
    (void)tokens;
    (void)valuep;
    return -1;
}
