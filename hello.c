#include <stdio.h>

char* const HI = "hi";

void printhi() {
    printf("hi\n");
}

int main() {
    printf("hi at: %p\n", HI);
    printf("printhi located at: %p\n", &printhi);
    for (int i = 0; i < 5; ++i)
        printhi();
    return 0;
}
