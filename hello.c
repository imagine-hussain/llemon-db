#include <stdio.h>

void printhi() {
    printf("hi\n");
}

int main() {
    printf("printhi located at: %p\n", &printhi);
    for (int i = 0; i < 5; ++i)
        printhi();
    return 0;
}
