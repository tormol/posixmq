/* A program to test when posix message queue descriptors are closed on exec */

#define _POSIX_C_SOURCE 200809L // for O_CLOEXEC
#define _NETBSD_SOURCE // for F_DUPFD_CLOEXEC on NetBSD
#include <fcntl.h>
#include <unistd.h>
#include <errno.h>
#include <string.h>
#include <stdlib.h>
#include <stdio.h>
#include <mqueue.h>

void test(char *progname, int d, char *testname) {
    if (d == -1) {
        fprintf(stderr, "creating %s failed: %s\n", testname, strerror(errno));
        exit(1);
    }
    char buf[6];
    switch (fork()) {
        case -1:
            fprintf(stderr, "fork failed: %s\n", strerror(errno));
            exit(1);
        case 0:
            return;
        default:
            sprintf(&buf[0], "%d", d);
            execl(progname, progname, testname, &buf[0], NULL);
            fprintf(stderr, "exec'ing %s failed: %s\n", progname, strerror(errno));
            exit(1);
    }
}

int main(int argc, char **argv) {
    if (argc == 3) {// after exec'ing
        int d = atoi(argv[2]);
        char *is_cloexec = (fcntl(d, F_GETFD) & FD_CLOEXEC) != 0 ? "yes" : "no";
        int error = mq_send(d, "send\n", 5, 2) == -1 ? errno : 0;
        fprintf(stderr, "%s (fd %d): is cloexec: %s, mq_send() result: %s\n",
            argv[1], d, is_cloexec, strerror(error)
        );
        return error;
    }

    mq_unlink("/test_cloexec");
    int without = mq_open("/test_cloexec", O_RDWR | O_CREAT, 0644, NULL);
    test(argv[0], without, "without O_CLOEXEC");
    int with = mq_open("/test_cloexec", O_RDWR | O_CLOEXEC, 0644, NULL);
    test(argv[0], with, "with O_CLOEXEC");

    test(argv[0], fcntl(without, F_DUPFD_CLOEXEC, 0), "cloned with F_DUPFD_CLOEXEC");
    test(argv[0], dup(with), "dup()'d");

    if (fcntl(without, F_SETFD, fcntl(without, F_GETFD)|FD_CLOEXEC) == -1) {
       fprintf(stderr, "enabling cloexec failed: %s\n", strerror(errno));
       return 1;
    }
    test(argv[0], without, "enabled FD_CLOEXEC");
    if (fcntl(with, F_SETFD, fcntl(with, F_GETFD)&~FD_CLOEXEC) == -1) {
       fprintf(stderr, "disabling cloexec failed: %s\n", strerror(errno));
       return 1;
    }
    test(argv[0], with, "cleared FD_CLOEXEC");

    return 0;
}
