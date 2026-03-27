#ifndef _SYS_WAIT_H
#define _SYS_WAIT_H

#include <sys/types.h>

#define WNOHANG     1
#define WUNTRACED   2
#define WSTOPPED    WUNTRACED
#define WEXITED     4
#define WCONTINUED  8
#define WNOWAIT     0x01000000

#define __WNOTHREAD 0x20000000
#define __WALL      0x40000000
#define __WCLONE    0x80000000

#define WEXITSTATUS(status)   (((status) >> 8) & 0xff)
#define WSTOPSIG(status)      (((status) >> 8) & 0xff)
#define WTERMSIG(status)      ((status) & 0x7f)
#define WCOREDUMP(status)     ((status) & 0x80)
#define WIFEXITED(status)     (WTERMSIG(status) == 0)
#define WIFSTOPPED(status)    (WSTOPSIG(status) != 0 && (((status) >> 8) & 0xff) == 0x7f)
#define WIFSIGNALED(status)   (!WIFSTOPPED(status) && !WIFEXITED(status))
#define WIFCONTINUED(status)  ((status) == 0xffff)

pid_t wait(int* status);
pid_t waitpid(pid_t pid, int* status, int options);
int waitid(idtype_t idtype, id_t id, siginfo_t* status, int options);

pid_t wait3(int* status, int options, struct rusage* rusage);
pid_t wait4(pid_t pid, int* status, int options, struct rusage* rusage);

#endif /* _SYS_WAIT_H */
