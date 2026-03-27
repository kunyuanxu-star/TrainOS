#ifndef _SYS_TYPES_H
#define _SYS_TYPES_H

#include <stddef.h>
#include <stdint.h>

typedef int8_t   caddr_t;
typedef int32_t  daddr_t;
typedef uint32_t dev_t;
typedef uint32_t gid_t;
typedef uint32_t ino_t;
typedef int32_t  key_t;
typedef int32_t  mode_t;
typedef uint32_t nlink_t;
typedef int64_t  off_t;
typedef int32_t  pid_t;
typedef int32_t  ssize_t;
typedef int32_t  suseconds_t;
typedef int64_t  time_t;
typedef int32_t  uid_t;

typedef unsigned long useconds_t;

typedef int32_t blksize_t;
typedef int64_t blkcnt_t;

typedef uint32_t pthread_t;
typedef int32_t pthread_attr_t;
typedef int32_t pthread_mutex_t;
typedef int32_t pthread_mutexattr_t;
typedef int32_t pthread_cond_t;
typedef int32_t pthread_condattr_t;
typedef int32_t pthread_rwlock_t;
typedef int32_t pthread_rwlockattr_t;
typedef int32_t pthread_spinlock_t;
typedef int32_t pthread_barrier_t;
typedef int32_t pthread_barrierattr_t;

typedef int32_t clock_t;
typedef int32_t clockid_t;
typedef int32_t key_t;
typedef int32_t idtype_t;
typedef int32_t id_t;

#define NULL ((void*)0)

typedef uint8_t   u_int8_t;
typedef uint16_t  u_int16_t;
typedef uint32_t  u_int32_t;
typedef uint64_t  u_int64_t;
typedef int8_t    int8_t;
typedef int16_t   int16_t;
typedef int32_t   int32_t;
typedef int64_t   int64_t;

typedef unsigned char u_char;
typedef unsigned short u_short;
typedef unsigned int u_int;
typedef unsigned long u_long;

typedef uint32_t u_quad_t;
typedef int32_t quad_t;
typedef quad_t* qaddr_t;

typedef char* caddr_t;

typedef struct { int val[2]; } fd_set;

typedef unsigned long fsblkcnt_t;
typedef unsigned long fsfilcnt_t;

typedef int64_t suseconds_t;

struct timespec {
    time_t tv_sec;
    long tv_nsec;
};

struct timeval {
    time_t tv_sec;
    suseconds_t tv_usec;
};

struct __ucontext {
    unsigned long uc_flags;
    struct __ucontext* uc_link;
    unsigned long uc_stack[16];
    long uc_mcontext[128];
    unsigned long uc_sigmask;
};

typedef unsigned long nfds_t;

typedef unsigned long socket_t;

#endif /* _SYS_TYPES_H */
