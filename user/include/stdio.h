#ifndef _STDIO_H
#define _STDIO_H

#include <stdarg.h>
#include <stddef.h>

#define EOF (-1)

#define stdin  ((FILE*)0)
#define stdout ((FILE*)1)
#define stderr ((FILE*)2)

typedef struct _FILE FILE;

extern FILE* const stdin;
extern FILE* const stdout;
extern FILE* const stderr;

int printf(const char* fmt, ...);
int sprintf(char* str, const char* fmt, ...);
int snprintf(char* str, size_t size, const char* fmt, ...);

int vprintf(const char* fmt, va_list ap);
int vsprintf(char* str, const char* fmt, va_list ap);
int vsnprintf(char* str, size_t size, const char* fmt, va_list ap);

int puts(const char* s);
int putchar(int c);
int getchar(void);

int scanf(const char* fmt, ...);
int sscanf(const char* str, const char* fmt, ...);

ssize_t getline(char** lineptr, size_t* n, FILE* stream);
ssize_t getdelim(char** lineptr, size_t* n, int delim, FILE* stream);

void perror(const char* s);

size_t fread(void* ptr, size_t size, size_t nmemb, FILE* stream);
size_t fwrite(const void* ptr, size_t size, size_t nmemb, FILE* stream);

int fflush(FILE* stream);
int fclose(FILE* stream);
FILE* fopen(const char* path, const char* mode);
FILE* fdopen(int fd, const char* mode);

int fprintf(FILE* stream, const char* fmt, ...);
int vfprintf(FILE* stream, const char* fmt, va_list ap);

char* fgets(char* s, int n, FILE* stream);
int fputs(const char* s, FILE* stream);

int fgetc(FILE* stream);
int fputc(int c, FILE* stream);

void clearerr(FILE* stream);
int feof(FILE* stream);
int ferror(FILE* stream);
long ftell(FILE* stream);
int fseek(FILE* stream, long offset, int whence);
void rewind(FILE* stream);

typedef long fpos_t;

int fgetpos(FILE* stream, fpos_t* pos);
int fsetpos(FILE* stream, const fpos_t* pos);

void setbuf(FILE* stream, char* buf);
int setvbuf(FILE* stream, char* buf, int mode, size_t size);

#define SEEK_SET 0
#define SEEK_CUR 1
#define SEEK_END 2

#define BUFSIZ  8192
#define FILENAME_MAX 256
#define L_tmpnam 256

#define TMP_MAX  10000

#endif /* _STDIO_H */
