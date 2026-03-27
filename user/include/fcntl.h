#ifndef _FCNTL_H
#define _FCNTL_H

#include <sys/types.h>

#define O_ACCMODE    00000003
#define O_RDONLY         00
#define O_WRONLY         01
#define O_RDWR           02
#define O_CREAT        0100
#define O_EXCL         0200
#define O_NOCTTY       0400
#define O_TRUNC       01000
#define O_APPEND      02000
#define O_NONBLOCK    04000
#define O_DSYNC       010000
#define O_SYNC        04010000
#define O_RSYNC       04010000
#define O_DIRECTORY  0200000
#define O_NOFOLLOW   0400000
#define O_CLOEXEC   02000000

#define O_DIRECT     040000
#define O_LARGEFILE   0100000
#define O_NOATIME    04000000

#define AT_FDCWD            -100
#define AT_SYMLINK_NOFOLLOW   0x100
#define AT_REMOVEDIR         0x200
#define AT_SYMLINK_FOLLOW    0x400
#define AT_EACCESS            0x200
#define AT_EMPTY_PATH         0x1000
#define AT_NO_CLOEXEC         0x2000

#define F_DUPFD       0
#define F_GETFD       1
#define F_SETFD       2
#define F_GETFL       3
#define F_SETFL       4
#define F_GETLK       5
#define F_SETLK       6
#define F_SETLKW      7
#define F_SETOWN      8
#define F_GETOWN      9
#define F_SETSIG      10
#define F_GETSIG      11
#define F_SETLK64     12
#define F_SETLKW64    13
#define F_GETLK64     14
#define F_SETOWN_EX   15
#define F_GETOWN_EX   16
#define F_GETLK64     17
#define F_OFD_GETLK   18
#define F_OFD_SETLK   19
#define F_OFD_SETLKW  20

#define FD_CLOEXEC     1

#define F_RDLCK        0
#define F_WRLCK        1
#define F_UNLCK        2

#define F_EXLCK        4
#define F_SHARE         8
#define F_POSIX        16
#define F_FSLEVEL      32
#define F_SHARE_NOSTR  64
#define F_SHARE_FSHARED 128
#define F_SHARE_RSHARED 256
#define F_SHARE_WSHARED 512
#define F_SHARE_DENYRD 1024
#define F_SHARE_DENYWR 2048
#define F_SHARE_DENYRW 4096

struct flock {
    short l_type;
    short l_whence;
    off_t l_start;
    off_t l_len;
    pid_t l_pid;
};

struct flock64 {
    short l_type;
    short l_whence;
    off64_t l_start;
    off64_t l_len;
    pid_t l_pid;
};

int open(const char* path, int flags, ...);
int openat(int fd, const char* path, int flags, ...);
int creat(const char* path, mode_t mode);
int fcntl(int fd, int cmd, ...);
int dup(int fd);
int dup2(int fd, int fd2);
int dup3(int fd, int fd2, int flags);

int flock(int fd, int operation);

int posix_fadvise(int fd, off_t offset, off_t len, int advice);
int posix_fallocate(int fd, off_t offset, off_t len);

int sync_file_range(int fd, off64_t offset, off64_t nbytes, unsigned int flags);

#endif /* _FCNTL_H */
