#include <fcntl.h>
#include <errno.h>
#include <string.h>
#include <stdio.h>
#include <mqueue.h>

int main(int argc, char **argv) {
    char *name = "/flash";
    if (argc > 1) {
        name = argv[1];
    }

    mqd_t d = mq_open(name, O_RDWR|O_CREAT, 0600, NULL);
    if (d == -1) {
        fprintf(stderr, "opening failed: %s\n", strerror(errno));
        return 1;
    }
    //mq_close(d); // uncommenting this fixes it
    if (mq_unlink(name) == -1) {
        fprintf(stderr, "unlinking failed: %s\n", strerror(errno));
        return 2;
    }
    // sleep(1); has no effect
    d = mq_open(name, O_RDWR);
    if (d == -1 && errno != ENOENT) {
        fprintf(stderr,
            "opening right after unlinking did not fail with ENOENT: %s\n",
            strerror(errno)
        );
        return 3;
    }
    return 0;
}
