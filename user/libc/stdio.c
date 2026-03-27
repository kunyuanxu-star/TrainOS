// Minimal stdio implementation for TrainOS
// This is a simplified version - full implementation would be extensive

#include <stdarg.h>

extern void _putchar(int c);
extern int _write(int fd, const char* buf, int len);
extern void* _sbrk(intptr_t incr);

static char print_buf[256];
static int print_buf_pos = 0;

void _putchar_buf(int c) {
    if (print_buf_pos < 255) {
        print_buf[print_buf_pos++] = (char)c;
    }
}

void _flush_buf() {
    if (print_buf_pos > 0) {
        _write(1, print_buf, print_buf_pos);
        print_buf_pos = 0;
    }
}

static const char digits[] = "0123456789abcdef";

static void print_int(int val, int base) {
    if (val < 0) {
        _putchar_buf('-');
        val = -val;
    }

    char buf[32];
    int i = 0;
    if (val == 0) {
        buf[i++] = '0';
    } else {
        while (val > 0) {
            buf[i++] = digits[val % base];
            val /= base;
        }
    }

    while (i > 0) {
        _putchar_buf(buf[--i]);
    }
}

static void print_uint(unsigned int val, int base) {
    char buf[32];
    int i = 0;
    if (val == 0) {
        buf[i++] = '0';
    } else {
        while (val > 0) {
            buf[i++] = digits[val % base];
            val /= base;
        }
    }

    while (i > 0) {
        _putchar_buf(buf[--i]);
    }
}

static void print_long(long val, int base) {
    if (val < 0) {
        _putchar_buf('-');
        val = -val;
    }

    char buf[32];
    int i = 0;
    if (val == 0) {
        buf[i++] = '0';
    } else {
        while (val > 0) {
            buf[i++] = digits[val % base];
            val /= base;
        }
    }

    while (i > 0) {
        _putchar_buf(buf[--i]);
    }
}

static void print_ptr(void* ptr) {
    _putchar_buf('0');
    _putchar_buf('x');
    unsigned long val = (unsigned long)ptr;
    char buf[32];
    int i = 0;
    while (val > 0) {
        buf[i++] = digits[val % 16];
        val /= 16;
    }
    if (i == 0) {
        buf[i++] = '0';
    }
    while (i > 0) {
        _putchar_buf(buf[--i]);
    }
}

int printf(const char* fmt, ...) {
    va_list ap;
    va_start(ap, fmt);

    print_buf_pos = 0;

    while (*fmt) {
        if (*fmt == '%' && fmt[1]) {
            fmt++;
            switch (*fmt) {
                case 'c': {
                    int c = va_arg(ap, int);
                    _putchar_buf((char)c);
                    break;
                }
                case 's': {
                    const char* s = va_arg(ap, const char*);
                    while (*s) {
                        _putchar_buf(*s++);
                    }
                    break;
                }
                case 'd':
                case 'i': {
                    int val = va_arg(ap, int);
                    print_int(val, 10);
                    break;
                }
                case 'u': {
                    unsigned int val = va_arg(ap, unsigned int);
                    print_uint(val, 10);
                    break;
                }
                case 'x':
                case 'X': {
                    unsigned int val = va_arg(ap, unsigned int);
                    print_uint(val, 16);
                    break;
                }
                case 'p': {
                    void* ptr = va_arg(ap, void*);
                    print_ptr(ptr);
                    break;
                }
                case 'l': {
                    if (fmt[1] == 'd' || fmt[1] == 'i') {
                        fmt++;
                        long val = va_arg(ap, long);
                        print_long(val, 10);
                    } else if (fmt[1] == 'u') {
                        fmt++;
                        unsigned long val = va_arg(ap, unsigned long);
                        // Simplified - just print as hex
                        print_uint((unsigned int)val, 16);
                    } else if (fmt[1] == 'x') {
                        fmt++;
                        unsigned long val = va_arg(ap, unsigned long);
                        print_uint((unsigned int)val, 16);
                    } else {
                        _putchar_buf('%');
                        _putchar_buf('l');
                    }
                    break;
                }
                case 'z': {
                    if (fmt[1] == 'd') {
                        fmt++;
                        size_t val = va_arg(ap, size_t);
                        print_uint(val, 10);
                    } else if (fmt[1] == 'u') {
                        fmt++;
                        size_t val = va_arg(ap, size_t);
                        print_uint(val, 10);
                    } else if (fmt[1] == 'x') {
                        fmt++;
                        size_t val = va_arg(ap, size_t);
                        print_uint(val, 16);
                    } else {
                        _putchar_buf('%');
                        _putchar_buf('z');
                    }
                    break;
                }
                case '%':
                    _putchar_buf('%');
                    break;
                default:
                    _putchar_buf('%');
                    _putchar_buf(*fmt);
                    break;
            }
        } else if (*fmt == '\n') {
            _putchar_buf('\r');
            _putchar_buf('\n');
        } else {
            _putchar_buf(*fmt);
        }
        fmt++;
    }

    va_end(ap);
    _flush_buf();
    return print_buf_pos;
}

int puts(const char* s) {
    while (*s) {
        if (*s == '\n') {
            _putchar_buf('\r');
        }
        _putchar_buf(*s++);
    }
    _putchar_buf('\n');
    _flush_buf();
    return 0;
}

int putchar(int c) {
    _putchar_buf((char)c);
    _flush_buf();
    return c;
}

void putc(int c) {
    _putchar_buf((char)c);
}

int sprintf(char* str, const char* fmt, ...) {
    va_list ap;
    va_start(ap, fmt);

    int pos = 0;
    char buf[32];

    while (*fmt) {
        if (*fmt == '%' && fmt[1]) {
            fmt++;
            switch (*fmt) {
                case 'c': {
                    int c = va_arg(ap, int);
                    str[pos++] = (char)c;
                    break;
                }
                case 's': {
                    const char* s = va_arg(ap, const char*);
                    while (*s) {
                        str[pos++] = *s++;
                    }
                    break;
                }
                case 'd':
                case 'i': {
                    int val = va_arg(ap, int);
                    int i = 0;
                    if (val < 0) {
                        str[pos++] = '-';
                        val = -val;
                    }
                    if (val == 0) {
                        str[pos++] = '0';
                    } else {
                        char tmp[32];
                        while (val > 0) {
                            tmp[i++] = digits[val % 10];
                            val /= 10;
                        }
                        while (i > 0) {
                            str[pos++] = tmp[--i];
                        }
                    }
                    break;
                }
                case 'x':
                case 'X': {
                    unsigned int val = va_arg(ap, unsigned int);
                    int i = 0;
                    if (val == 0) {
                        str[pos++] = '0';
                    } else {
                        char tmp[32];
                        while (val > 0) {
                            tmp[i++] = digits[val % 16];
                            val /= 16;
                        }
                        while (i > 0) {
                            str[pos++] = tmp[--i];
                        }
                    }
                    break;
                }
                case 's':
                    break;
                default:
                    str[pos++] = '%';
                    str[pos++] = *fmt;
                    break;
            }
        } else {
            str[pos++] = *fmt;
        }
        fmt++;
    }

    str[pos] = '\0';
    va_end(ap);
    return pos;
}
