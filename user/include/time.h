#ifndef _TIME_H
#define _TIME_H

#include <stddef.h>

#define CLOCKS_PER_SEC 1000000

typedef long clock_t;
typedef long time_t;

struct tm {
    int tm_sec;
    int tm_min;
    int tm_hour;
    int tm_mday;
    int tm_mon;
    int tm_year;
    int tm_wday;
    int tm_yday;
    int tm_isdst;
    long tm_gmtoff;
    const char* tm_zone;
};

clock_t clock(void);
time_t time(time_t* t);
double difftime(time_t time1, time_t time2);
time_t mktime(struct tm* tm);
size_t strftime(char* s, size_t max, const char* fmt, const struct tm* tm);

char* asctime(const struct tm* tm);
char* ctime(const time_t* t);
struct tm* gmtime(const time_t* t);
struct tm* localtime(const time_t* t);

char* asctime_r(const struct tm* tm, char* buf);
char* ctime_r(const time_t* t, char* buf);
struct tm* gmtime_r(const time_t* t, struct tm* result);
struct tm* localtime_r(const time_t* t, struct tm* result);

int nanosleep(const struct timespec* req, struct timespec* rem);

struct timespec {
    time_t tv_sec;
    long tv_nsec;
};

struct itimerspec {
    struct timespec it_interval;
    struct timespec it_value;
};

#define TIMER_ABSTIME 1

int timer_create(clockid_t clockid, struct sigevent* evp, timer_t* timerid);
int timer_delete(timer_t timerid);
int timer_gettime(timer_t timerid, struct itimerspec* val);
int timer_getoverrun(timer_t timerid);
int timer_settime(timer_t timerid, int flags, const struct itimerspec* val,
                  struct itimerspec* old);

int clock_getres(clockid_t clockid, struct timespec* res);
int clock_gettime(clockid_t clockid, struct timespec* tp);
int clock_settime(clockid_t clockid, const struct timespec* tp);
int clock_nanosleep(clockid_t clockid, int flags, const struct timespec* req,
                    struct timespec* rem);

#define CLOCK_REALTIME  0
#define CLOCK_MONOTONIC 1
#define CLOCK_PROCESS_CPUTIME_ID 2
#define CLOCK_THREAD_CPUTIME_ID  3

#endif /* _TIME_H */
