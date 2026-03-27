#ifndef _CTYPE_H
#define _CTYPE_H

int isalnum(int c);
int isalpha(int c);
int isblank(int c);
int iscntrl(int c);
int isdigit(int c);
int isgraph(int c);
int islower(int c);
int isprint(int c);
int ispunct(int c);
int isspace(int c);
int isupper(int c);
int isxdigit(int c);
int tolower(int c);
int toupper(int c);

#define _U  01
#define _L  02
#define _D  04
#define _C  010
#define _P  020
#define _S  040
#define _X  0100

extern const unsigned char _ctype[];

#define isalnum(c) (_ctype[(c)+1] & (_U|_L|_D))
#define isalpha(c) (_ctype[(c)+1] & (_U|_L))
#define isblank(c) (_ctype[(c)+1] & _B)
#define iscntrl(c) (_ctype[(c)+1] & _C)
#define isdigit(c) (_ctype[(c)+1] & _D)
#define isgraph(c) (_ctype[(c)+1] & (_P|_U|_L|_D))
#define islower(c) (_ctype[(c)+1] & _L)
#define isprint(c) (_ctype[(c)+1] & (_P|_U|_L|_D|_B))
#define ispunct(c) (_ctype[(c)+1] & _P)
#define isspace(c) (_ctype[(c)+1] & _S)
#define isupper(c) (_ctype[(c)+1] & _U)
#define isxdigit(c) (_ctype[(c)+1] & (_D|_X))

#define tolower(c) ((c) + ('a' - 'A'))
#define toupper(c) ((c) - ('a' - 'A'))

#endif /* _CTYPE_H */
