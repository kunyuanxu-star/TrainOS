// Minimal string implementation for TrainOS

#include <stddef.h>

void* memcpy(void* dest, const void* src, size_t n) {
    unsigned char* d = (unsigned char*)dest;
    const unsigned char* s = (const unsigned char*)src;
    while (n--) {
        *d++ = *s++;
    }
    return dest;
}

void* memmove(void* dest, const void* src, size_t n) {
    unsigned char* d = (unsigned char*)dest;
    const unsigned char* s = (const unsigned char*)src;
    if (d < s) {
        while (n--) {
            *d++ = *s++;
        }
    } else {
        d += n;
        s += n;
        while (n--) {
            *--d = *--s;
        }
    }
    return dest;
}

void* memset(void* s, int c, size_t n) {
    unsigned char* p = (unsigned char*)s;
    unsigned char uc = (unsigned char)c;
    while (n--) {
        *p++ = uc;
    }
    return s;
}

int memcmp(const void* s1, const void* s2, size_t n) {
    const unsigned char* p1 = (const unsigned char*)s1;
    const unsigned char* p2 = (const unsigned char*)s2;
    while (n--) {
        if (*p1 != *p2) {
            return *p1 - *p2;
        }
        p1++;
        p2++;
    }
    return 0;
}

void* memchr(const void* s, int c, size_t n) {
    const unsigned char* p = (const unsigned char*)s;
    unsigned char uc = (unsigned char)c;
    while (n--) {
        if (*p == uc) {
            return (void*)p;
        }
        p++;
    }
    return NULL;
}

size_t strlen(const char* s) {
    size_t n = 0;
    while (s[n]) {
        n++;
    }
    return n;
}

size_t strnlen(const char* s, size_t maxlen) {
    size_t n = 0;
    while (n < maxlen && s[n]) {
        n++;
    }
    return n;
}

char* strcpy(char* dest, const char* src) {
    char* d = dest;
    while ((*d++ = *src++))
        ;
    return dest;
}

char* strncpy(char* dest, const char* src, size_t n) {
    char* d = dest;
    size_t i = 0;
    while (i < n && *src) {
        *d++ = *src++;
        i++;
    }
    while (i < n) {
        *d++ = '\0';
        i++;
    }
    return dest;
}

char* strcat(char* dest, const char* src) {
    char* d = dest;
    while (*d) {
        d++;
    }
    while ((*d++ = *src++))
        ;
    return dest;
}

char* strncat(char* dest, const char* src, size_t n) {
    char* d = dest;
    while (*d) {
        d++;
    }
    size_t i = 0;
    while (i < n && *src) {
        *d++ = *src++;
        i++;
    }
    *d = '\0';
    return dest;
}

int strcmp(const char* s1, const char* s2) {
    while (*s1 && *s2) {
        if (*s1 != *s2) {
            return (unsigned char)*s1 - (unsigned char)*s2;
        }
        s1++;
        s2++;
    }
    return (unsigned char)*s1 - (unsigned char)*s2;
}

int strncmp(const char* s1, const char* s2, size_t n) {
    while (n-- && *s1 && *s2) {
        if (*s1 != *s2) {
            return (unsigned char)*s1 - (unsigned char)*s2;
        }
        s1++;
        s2++;
    }
    return (unsigned char)*s1 - (unsigned char)*s2;
}

char* strchr(const char* s, int c) {
    char c1 = (char)c;
    while (*s) {
        if (*s == c1) {
            return (char*)s;
        }
        s++;
    }
    return NULL;
}

char* strrchr(const char* s, int c) {
    char c1 = (char)c;
    const char* last = NULL;
    while (*s) {
        if (*s == c1) {
            last = s;
        }
        s++;
    }
    return (char*)last;
}

char* strstr(const char* haystack, const char* needle) {
    if (!*needle) {
        return (char*)haystack;
    }
    while (*haystack) {
        const char* h = haystack;
        const char* n = needle;
        while (*h && *n && *h == *n) {
            h++;
            n++;
        }
        if (!*n) {
            return (char*)haystack;
        }
        haystack++;
    }
    return NULL;
}

size_t strcspn(const char* s, const char* reject) {
    size_t n = 0;
    while (*s) {
        const char* r = reject;
        while (*r) {
            if (*s == *r) {
                return n;
            }
            r++;
        }
        s++;
        n++;
    }
    return n;
}

size_t strspn(const char* s, const char* accept) {
    size_t n = 0;
    while (*s) {
        const char* a = accept;
        while (*a) {
            if (*s == *a) {
                break;
            }
            a++;
        }
        if (!*a) {
            return n;
        }
        s++;
        n++;
    }
    return n;
}

char* strpbrk(const char* s, const char* accept) {
    while (*s) {
        const char* a = accept;
        while (*a) {
            if (*s == *a) {
                return (char*)s;
            }
            a++;
        }
        s++;
    }
    return NULL;
}

char* strtok_r(char* str, const char* delim, char** saveptr) {
    char* token;
    if (str == NULL) {
        str = *saveptr;
    }
    if (str == NULL) {
        return NULL;
    }

    str += strspn(str, delim);
    if (!*str) {
        *saveptr = NULL;
        return NULL;
    }
    token = str;
    str += strcspn(str, delim);
    if (*str) {
        *str = '\0';
        str++;
    }
    *saveptr = str;
    return token;
}

char* strtok(char* str, const char* delim) {
    static char* last;
    return strtok_r(str, delim, &last);
}

int strcasecmp(const char* s1, const char* s2) {
    while (*s1 && *s2) {
        char c1 = *s1;
        char c2 = *s2;
        if (c1 >= 'A' && c1 <= 'Z') {
            c1 = c1 - 'A' + 'a';
        }
        if (c2 >= 'A' && c2 <= 'Z') {
            c2 = c2 - 'A' + 'a';
        }
        if (c1 != c2) {
            return c1 - c2;
        }
        s1++;
        s2++;
    }
    return (unsigned char)*s1 - (unsigned char)*s2;
}

int strncasecmp(const char* s1, const char* s2, size_t n) {
    while (n-- && *s1 && *s2) {
        char c1 = *s1;
        char c2 = *s2;
        if (c1 >= 'A' && c1 <= 'Z') {
            c1 = c1 - 'A' + 'a';
        }
        if (c2 >= 'A' && c2 <= 'Z') {
            c2 = c2 - 'A' + 'a';
        }
        if (c1 != c2) {
            return c1 - c2;
        }
        s1++;
        s2++;
    }
    return (unsigned char)*s1 - (unsigned char)*s2;
}

void* memccpy(void* dest, const void* src, int c, size_t n) {
    unsigned char* d = (unsigned char*)dest;
    const unsigned char* s = (const unsigned char*)src;
    while (n--) {
        *d++ = *s++;
        if (d[-1] == (unsigned char)c) {
            return d;
        }
    }
    return NULL;
}
