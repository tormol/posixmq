/* Repro for NetBSD bug which makes all future mq_open()s fail with EMFILE even if none are open */

#include <mqueue.h>
#include <errno.h>
#include <stdio.h>

int main(void) {
    mqd_t mq;
    if (mq_unlink("/emfile") == -1) {
        if (errno != ENOENT) {
            perror("unlink");
            return 1;
        }
    }

    if ((mq = mq_open("/emfile", O_RDWR | O_CREAT, 0600, NULL)) == -1) {
        perror("first open");
        return 1;
    }
    if (mq_close(mq) == -1) {
        perror("first close");
        return 1;
    }

    if ((mq = mq_open("/emfile", O_RDWR | O_CREAT, 0600, NULL)) == -1) {
        perror("second open");
        return 1;
    }
    if (mq_close(mq) == -1) {
        perror("second close");
        return 1;
    }

    /* This fails with EMFILE */
    if ((mq = mq_open("/emfile", O_RDWR | O_CREAT, 0600, NULL)) == -1) {
        perror("third open");
        return 1;
    }
    if (mq_close(mq) == -1) {
        perror("third close");
        return 1;
    }

    return 0;
}
