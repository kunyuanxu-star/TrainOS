#ifndef _UNISTD_H
#define _UNISTD_H

#include <stddef.h>

#define STDIN_FILENO  0
#define STDOUT_FILENO 1
#define STDERR_FILENO 2

#define R_OK 4
#define W_OK 2
#define X_OK 1
#define F_OK 0

#define F_ULOCK 0
#define F_LOCK  1
#define F_TLOCK 2
#define F_TEST  3

void _exit(int status) __attribute__((noreturn));

int access(const char* path, int amode);
unsigned int alarm(unsigned int seconds);
int brk(void* addr);
void* sbrk(intptr_t increment);

int chdir(const char* path);
int fchdir(int fd);
int chmod(const char* path, mode_t mode);
int fchmod(int fd, mode_t mode);
int fchmodat(int fd, const char* path, mode_t mode, int flag);

int chown(const char* path, uid_t owner, gid_t group);
int fchown(int fd, uid_t owner, gid_t group);
int lchown(const char* path, uid_t owner, gid_t group);

int close(int fd);

int dup(int fd);
int dup2(int fd, int fd2);
int dup3(int fd, int fd2, int flags);

long faccessat(int fd, const char* path, int mode, int flag);

int fadvise(int fd, off_t offset, off_t len, int advice);
int fallocate(int fd, int mode, off_t offset, off_t len);

int fchownat(int fd, const char* path, uid_t owner, gid_t group, int flag);

pid_t fork(void);
pid_t vfork(void);

int fsync(int fd);
int fdatasync(int fd);

char* getcwd(char* buf, size_t size);
char* get_current_dir_name(void);

gid_t getegid(void);
uid_t geteuid(void);
gid_t getgid(void);
uid_t getuid(void);

int gethostname(char* name, size_t len);
char* getlogin(void);
char* getlogin_r(char* name, size_t len);
int getpagesize(void);

char* getpass(const char* prompt);

int getpeername(int sockfd, struct sockaddr* addr, socklen_t* addrlen);
int getsockname(int sockfd, struct sockaddr* addr, socklen_t* addrlen);
int getsockopt(int sockfd, int level, int optname, void* optval, socklen_t* optlen);
int setsockopt(int sockfd, int level, int optname, const void* optval, socklen_t optlen);

int isatty(int fd);

int kill(pid_t pid, int sig);
int tkill(int tid, int sig);
int tgkill(int pid, int tid, int sig);

int link(const char* old, const char* new);
int linkat(int oldfd, const char* old, int newfd, const char* new, int flags);
int symlink(const char* target, const char* linkpath);
int symlinkat(const char* target, int newfd, const char* linkpath);
ssize_t readlink(char* buf, size_t bufsiz);
ssize_t readlinkat(int fd, const char* path, char* buf, size_t bufsiz);

off_t lseek(int fd, off_t offset, int whence);
int fsseek(int fd, off_t offset, int whence);
off_t ftell(int fd);
int fgetpos(FILE* stream, fpos_t* pos);
int fsetpos(FILE* stream, const fpos_t* pos);

long pathconf(const char* path, int name);
long fpathconf(int fd, int name);

int pause(void);
int pipe(int pipefd[2]);
int pipe2(int pipefd[2], int flags);

ssize_t pread(int fd, void* buf, size_t count, off_t offset);
ssize_t pwrite(int fd, const void* buf, size_t count, off_t offset);

ssize_t read(int fd, void* buf, size_t count);
ssize_t write(int fd, const void* buf, size_t count);

ssize_t readv(int fd, const struct iovec* iov, int iovcnt);
ssize_t writev(int fd, const struct iovec* iov, int iovcnt);

ssize_t preadv(int fd, const struct iovec* iov, int iovcnt, off_t offset);
ssize_t pwritev(int fd, const struct iovec* iov, int iovcnt, off_t offset);

void* mmap(void* addr, size_t len, int prot, int flags, int fd, off_t offset);
int munmap(void* addr, size_t len);
int mprotect(void* addr, size_t len, int prot);
int msync(void* addr, size_t len, int flags);
int mlock(const void* addr, size_t len);
int munlock(const void* addr, size_t len);
int mlockall(int flags);
int munlockall(void);

int poll(struct pollfd* fds, nfds_t nfds, int timeout);
int ppoll(struct pollfd* fds, nfds_t nfds, const struct timespec* timeout_ts,
          const sigset_t* sigmask);

int preadv2(int fd, const struct iovec* iov, int iovcnt, off_t offset, int flags);
int pwritev2(int fd, const struct iovec* iov, int iovcnt, off_t offset, int flags);

ssize_t readlinkat(int fd, const char* path, char* buf, size_t bufsiz);

int rename(const char* old, const char* new);
int renameat(int oldfd, const char* old, int newfd, const char* new);
int renameat2(int oldfd, const char* old, int newfd, const char* new, unsigned int flags);

int rmdir(const char* path);

int setpgid(pid_t pid, pid_t pgid);
pid_t getpgid(pid_t pid);
pid_t getpgrp(void);
int setpgrp(void);
pid_t getsid(pid_t pid);
pid_t setsid(pid_t pid);

int setreuid(uid_t ruid, uid_t euid);
int setregid(gid_t rgid, gid_t egid);
int setuid(uid_t uid);
int setgid(gid_t gid);

unsigned int sleep(unsigned int seconds);
int usleep(useconds_t useconds);

long syscall(long sysno, ...);

int shutdown(int sockfd, int how);
int socket(int domain, int type, int protocol);
int socketpair(int domain, int type, int protocol, int sv[2]);
int bind(int sockfd, const struct sockaddr* addr, socklen_t addrlen);
int connect(int sockfd, const struct sockaddr* addr, socklen_t addrlen);
int listen(int sockfd, int backlog);
int accept(int sockfd, struct sockaddr* addr, socklen_t* addrlen);
int accept4(int sockfd, struct sockaddr* addr, socklen_t* addrlen, int flags);

int send(int sockfd, const void* buf, size_t len, int flags);
int recv(int sockfd, void* buf, size_t len, int flags);
int sendto(int sockfd, const void* buf, size_t len, int flags,
           const struct sockaddr* dest_addr, socklen_t addrlen);
int recvfrom(int sockfd, void* buf, size_t len, int flags,
             struct sockaddr* src_addr, socklen_t* addrlen);
int sendmsg(int sockfd, const struct msghdr* msg, int flags);
int recvmsg(int sockfd, struct msghdr* msg, int flags);

struct iovec {
    void* iov_base;
    size_t iov_len;
};

struct msghdr {
    void* msg_name;
    socklen_t msg_namelen;
    struct iovec* msg_iov;
    int msg_iovlen;
    void* msg_control;
    socklen_t msg_controllen;
    int msg_flags;
};

struct cmsghdr {
    socklen_t cmsg_len;
    int cmsg_level;
    int cmsg_type;
};

#define CMSG_FIRSTHDR(msg) ((msg)->msg_controllen >= sizeof(struct cmsghdr) ? \
                            (struct cmsghdr*)((msg)->msg_control) : \
                            (struct cmsghdr*)0)

#define CMSG_DATA(cmsg) ((unsigned char*)((cmsg) + 1))
#define CMSG_NXTHDR(msg, cmsg) ((cmsg)->cmsg_len >= sizeof(struct cmsghdr) ? \
                                (struct cmsghdr*)((unsigned char*)(cmsg) + \
                                (((cmsg)->cmsg_len + sizeof(size_t) - 1) & ~(sizeof(size_t) - 1))) : \
                                (struct cmsghdr*)0)

int symlink(const char* target, const char* linkpath);

mode_t umask(mode_t mask);
mode_t getumask(void);

int unlink(const char* path);
int unlinkat(int fd, const char* path, int flags);

int truncate(const char* path, off_t length);
int ftruncate(int fd, off_t length);

pid_t wait(int* status);
pid_t waitpid(pid_t pid, int* status, int options);
int waitid(idtype_t idtype, id_t id, siginfo_t* info, int options);

unsigned long alarm(unsigned int seconds);

useconds_t ualarm(useconds_t value, useconds_t interval);

int setvbuf(FILE* stream, char* buf, int mode, size_t size);
void setbuf(FILE* stream, char* buf);

int fileno(FILE* stream);
FILE* fdopen(int fd, const char* mode);

int fseeko(FILE* stream, off_t offset, int whence);
off_t ftello(FILE* stream);

int getopt(int argc, char* const argv[], const char* optstring);
extern char* optarg;
extern int optind, opterr, optopt;

char* ttyname(int fd);
int ttyname_r(int fd, char* buf, size_t buflen);

long getsubopt(char** optionp, char* const* tokens, char** valuep);

int tcgetattr(int fd, struct termios* termios_p);
int tcsetattr(int fd, int action, const struct termios* termios_p);

int tcflow(int fd, int action);
int tcflush(int fd, int queue_selector);
int tcgetpgrp(int fd);
int tcsetpgrp(int fd, pid_t pgrp);

pid_t gettid(void);

int getdomainname(char* name, size_t len);
int setdomainname(const char* name, size_t len);
int sethostname(const char* name, size_t len);

long gethostid(void);
int sethostid(long hostid);

int gettimeofday(struct timeval* tv, struct timezone* tz);
int settimeofday(const struct timeval* tv, const struct timezone* tz);

struct timeval {
    time_t tv_sec;
    suseconds_t tv_usec;
};

struct timezone {
    int tz_minuteswest;
    int tz_dsttime;
};

int utimes(const char* path, const struct timeval times[2]);
int futimes(int fd, const struct timeval times[2]);

struct timespec {
    time_t tv_sec;
    long tv_nsec;
};

int lutimes(const char* path, const struct timeval times[2]);

int vhangup(void);
int revoke(const char* path);

int profil(unsigned short* buf, size_t bufsiz, unsigned long offset, unsigned int scale);

int acct(const char* path);

struct perf_event_attr;

int perf_event_open(struct perf_event_attr* attr, pid_t pid, int cpu, int group_fd, unsigned long flags);

int reboot(int cmd);
int init_module(void* module_image, unsigned long len, const char* param_values);
int delete_module(const char* name, int flags);

#endif /* _UNISTD_H */
