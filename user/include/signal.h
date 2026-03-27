#ifndef _SIGNAL_H
#define _SIGNAL_H

#include <sys/types.h>

#define SIG_DFL ((void (*)(int))0)
#define SIG_IGN ((void (*)(int))1)
#define SIG_ERR ((void (*)(int))-1)

#define SIGINT      2
#define SIGILL      4
#define SIGABRT     6
#define SIGFPE      8
#define SIGSEGV     11
#define SIGTERM     15
#define SIGSTKFLT   16
#define SIGCHLD     17
#define SIGCONT     18
#define SIGSTOP     19
#define SIGTSTP     20
#define SIGTTIN     21
#define SIGTTOU     22
#define SIGURG      23
#define SIGXCPU     24
#define SIGXFSZ     25
#define SIGVTALRM   26
#define SIGPROF     27
#define SIGWINCH    28
#define SIGIO       29
#define SIGPOLL     29
#define SIGPWR      30
#define SIGSYS      31

#define NSIG       32

typedef int sig_atomic_t;

union sigval {
    int sival_int;
    void* sival_ptr;
};

typedef struct {
    int si_signo;
    int si_code;
    int si_errno;
    int si_pad0;
    pid_t si_pid;
    uid_t si_uid;
    void* si_addr;
    int si_pad1[2];
    union sigval si_value;
    int si_status;
    long si_band;
} siginfo_t;

struct sigaction {
    union {
        void (*sa_handler)(int);
        void (*sa_sigaction)(int, siginfo_t*, void*);
    };
    int sa_flags;
    void (*sa_restorer)(void);
    sigset_t sa_mask;
};

typedef void (*sighandler_t)(int);

sighandler_t signal(int signum, sighandler_t handler);
int raise(int sig);
int kill(pid_t pid, int sig);
int tkill(int tid, int sig);
int tgkill(int pid, int tid, int sig);

int sigaction(int signum, const struct sigaction* act, struct sigaction* oldact);
int sigpending(sigset_t* set);
int sigprocmask(int how, const sigset_t* set, sigset_t* oldset);
int sigsuspend(const sigset_t* mask);
int sigwait(const sigset_t* set, int* sig);
int sigwaitinfo(const sigset_t* set, siginfo_t* info);
int sigtimedwait(const sigset_t* set, siginfo_t* info, const struct timespec* timeout);
int sigqueue(pid_t pid, int sig, const union sigval value);

int sigaltstack(const stack_t* ss, stack_t* oss);

typedef struct {
    void* ss_sp;
    int ss_flags;
    size_t ss_size;
} stack_t;

#define SS_ONSTACK   1
#define SS_DISABLE   2

#define MINSIGSTKSZ  2048
#define SIGSTKSZ     8192

typedef unsigned long sigset_t;

int sigemptyset(sigset_t* set);
int sigfillset(sigset_t* set);
int sigaddset(sigset_t* set, int signum);
int sigdelset(sigset_t* set, int signum);
int sigismember(const sigset_t* set, int signum);

#define SIG_BLOCK     0
#define SIG_UNBLOCK   1
#define SIG_SETMASK   2

void psignal(int sig, const char* s);

#endif /* _SIGNAL_H */
