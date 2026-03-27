#include <stdio.h>
#include <stdlib.h>

int main(int argc, char* argv[]) {
    printf("Hello from TrainOS C program!\n");
    printf(" argc = %d\n", argc);
    for (int i = 0; i < argc; i++) {
        printf(" argv[%d] = %s\n", i, argv[i]);
    }
    printf("Goodbye!\n");
    return 0;
}
