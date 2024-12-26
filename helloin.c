#include <stdio.h>

char const* const HI = "hi";

void printhi() { printf("hi\n"); }

int main() {
  printf("hi at: %p\n", HI);
  for (int i = 0; i < 5; ++i)
    printhi();
  return 0;
}
