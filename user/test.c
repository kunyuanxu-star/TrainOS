#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/stat.h>
#include <fcntl.h>

void test_printf() {
    printf("=== printf test ===\n");
    printf("Integer: %d\n", 42);
    printf("Hex: 0x%x\n", 0xDEADBEEF);
    printf("String: %s\n", "Hello TrainOS");
    printf("Char: %c\n", 'X');
    printf("Unsigned: %u\n", 4294967295UL);
    printf("Long: %ld\n", 1234567890L);
    printf("Double: %f\n", 3.14159);
    printf("Percent: 100%%\n");
}

void test_memory() {
    printf("\n=== Memory test ===\n");
    char* s1 = "Hello";
    char* s2 = "World";
    char* s3 = (char*)malloc(100);
    if (!s3) {
        printf("malloc failed!\n");
        return;
    }
    strcpy(s3, "Allocated memory works!");
    printf("s1: %s\n", s1);
    printf("s2: %s\n", s2);
    printf("s3: %s\n", s3);
    free(s3);
    printf("Memory test passed!\n");
}

void test_file() {
    printf("\n=== File operations test ===\n");
    char filename[] = "/testfile.txt";
    int fd = open(filename, O_WRONLY | O_CREAT | O_TRUNC, 0644);
    if (fd >= 0) {
        const char* msg = "Hello from C file test!\n";
        write(fd, msg, strlen(msg));
        close(fd);
        printf("File written: %s\n", filename);
    } else {
        printf("File open failed (expected in userspace)\n");
    }
}

void test_syscalls() {
    printf("\n=== Syscall test ===\n");
    printf("getpid() = %d\n", getpid());
    printf("getuid() = %d\n", getuid());
    printf("getgid() = %d\n", getgid());
    printf("getpagesize() = %d\n", getpagesize());
}

int main(int argc, char* argv[]) {
    printf("TrainOS C Test Program\n");
    printf("======================\n\n");

    test_printf();
    test_memory();
    test_file();
    test_syscalls();

    printf("\nAll tests completed!\n");
    return 0;
}
