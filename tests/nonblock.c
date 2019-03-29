#include <mqueue.h>
#include <errno.h>
#include <stdio.h>

int main(void) {
    mqd_t mq;
    char buf[8192];
    unsigned int prio;
    if ((mq = mq_open("/nonblock", O_RDONLY | O_CREAT | O_NONBLOCK, 0600, NULL)) == -1) {
        perror("open");
        return 1;
    }
    if (mq_receive(mq, buf, 8192, &prio) == -1) {
        perror("receive");
    } else {
        printf("receive succeeded.\n");
    }
    if (mq_close(mq) == -1) {
        perror("close");
        return 1;
    }
    return 0;
}
