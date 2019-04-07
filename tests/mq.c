/* Copyright 2019 Torbjørn Birch Moltu
 *
 * Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
 * http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
 * http://opensource.org/licenses/MIT>, at your option. This file may not be
 * copied, modified, or distributed except according to those terms.
 */

// A command line program for interacting with posix message queues.
// Wraps the C functions thinly to expose as many error conditions as possible.
// Compile with gcc -o mq mq.c -lrt -std=c11 -Wall -Wextra -Wpedantic -g

#define _POSIX_C_SOURCE 200809L // needed for O_CLOEXEC
#include <mqueue.h>
#include <time.h>
#include <unistd.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <dirent.h>
#include <errno.h>
#include <string.h>
#include <stdio.h>
#include <stdlib.h>

void printerr(int err, char* action, char*(*specific)(int), int ex) __attribute__((noreturn));
void printerr(int err, char* action, char*(*specific)(int), int ex) {
    fprintf(stderr, "%s failed with errno %d = %s\n(generic desc: %s)\n",
        action, err, specific(err), strerror(err)
    );
    exit(ex);
}

void usage() __attribute__((noreturn));
void usage() {
    fprintf(stderr, "mq - work with POSIX message queues\n");
    fprintf(stderr, "Usage:\n");
    fprintf(stderr, "\tmq ls : list all existing queues\n\t\t(uses /dev/mqueue/)\n");
    fprintf(stderr, "\tmq rm /mqname... : mq_unlink() wrapper\n\t\tsupports multiple queues\n");
    fprintf(stderr, "\tmq stat (/mqname openmode)... : mq_getattr() wrapper\n\t\tsupports multiple queues\n");
    fprintf(stderr, "\tmq read /mqname openmode : call mq_receive() once\n");
    fprintf(stderr, "\t\tprints priority before the message content\n");
    fprintf(stderr, "\tmq read /mqname openmode timeout: call mq_timedreceive() once\n");
    fprintf(stderr, "\t\ttimeout is in seconds and must be an integer\n");
    fprintf(stderr, "\tmq write /mqname openmode priority message: call mq_send() once\n");
    fprintf(stderr, "\tmq write /mqname openmode priority message timeout: call mq_timedsend() once\n");
    fprintf(stderr, "openmode format: flags[perms][,capacity,size]\n");
    fprintf(stderr, "\tflags: r=O_RDONLY, w=O_WRONLY, d=O_RDWR, c=O_CREAT, e=O_EXCL\n");
    fprintf(stderr, "\t       n=O_NONBLOCK, s=O_CLOEXEC\n");
    fprintf(stderr, "\tIf there is only a single number it is used for permissions,\n");
    fprintf(stderr, "\tif there are two they are used for capacity and size limit.\n");
    fprintf(stderr, "\tExamples: 'd' 'wcn8,1024' 'rce700' 'rce733,10,200'\n");
    exit(1);
}

char* openerrdesc(int err) {
    switch (err) {
        case EACCES: return "EACCES: not permitted to open in this mode, or, more than one '/' in name";
        case EINVAL: return "EINVAL: invalid capacities, or, no slash in name";
        case ENOENT: return "ENOENT: queue doesn't exist, or, name is just '/'";
        case ENAMETOOLONG: return "ENAMETOOLONG - self explanatory";
        case EEXIST: return "EEXIST: queue already exists";
        case EMFILE: return "per-process fd limit reached";
        case ENFILE: return "system-wide fd limit reached";
        case ENOMEM: return "ENOMEM: process out of memory";
        case ENOSPC: return "ENOSPC: system out of memory";
        default: return "undocumented error!";
    }
}

mqd_t parseopts_open(char* qname, char* qopts) {
    int opts = 0;
    while (1) {
        int badopt = 0;
        switch (*qopts) {
            case 'r': opts |= O_RDONLY; break;
            case 'w': opts |= O_WRONLY; break;
            case 'b': opts |= O_RDWR; break;
            case 'c': opts |= O_CREAT; break;
            case 'e': opts |= O_EXCL; break;
            case 'n': opts |= O_NONBLOCK; break;
            case 's': opts |= O_CLOEXEC; break;
            default: badopt = 1; break;
        }
        if (badopt) {
            break;
        }
        qopts++;
    }
    
    char* numstarts[3] = {NULL, NULL, NULL};
    int nums = 0;
    int in_num = 0;
    while (*qopts != '\0') {
        if (*qopts >= '0' && *qopts <= '9') {
            if (!in_num) {
                if (nums == 3) {
                    fprintf(stderr, "Too many numbers in open options\n");
                    exit(1);
                }
                numstarts[nums] = qopts;
                nums++;
                in_num = 1;
            }
        } else if (*qopts == ',') {
            if (!in_num) {
               fprintf(stderr, "Empty number in open options\n");
               exit(1);
            }
            in_num = 0;
        } else if (nums == 0 && !in_num) {
            fprintf(stderr, "Invalid open mode %c\n", *qopts);
            exit(1);
        } else {
            fprintf(stderr, "mode flags must come before other open options\n");
            exit(1);
        }
        qopts++;
    }

    int perms = 0640;
    struct mq_attr caps;
    struct mq_attr* caps_ptr = NULL;
    if (nums%2 != 0) {
        perms = (int)strtoul(numstarts[0], NULL, 8);
    }
    if (nums >= 2) {
        caps.mq_maxmsg = atoi(numstarts[nums-2]);
        caps.mq_msgsize = atoi(numstarts[nums-1]);
        caps_ptr = &caps;
    }

    mqd_t q = mq_open(qname, opts, perms, caps_ptr);
    if (q == (mqd_t)-1) {
        printerr(errno, "opening", openerrdesc, 1);
    }
    return q;
}

struct timespec parse_timeout(char *timeout) {
    struct timespec deadline;
    if (clock_gettime(CLOCK_REALTIME, &deadline)) {
        perror("Unable to get current system time.");
        exit(1);
    }
    deadline.tv_sec += atoi(timeout);
    return deadline;
}


char* recverrdesc(int err) {
    switch (err) {
        case EAGAIN: return "EAGAIN: queue is empty so the call would have to block";
        case EBADF: return "EBADF: BUG!";
        case EINTR: return "EINTR: interrupted; try again";
        case EMSGSIZE: return "EMSGSIZE: the receive buffer is smaller than the maximum message size";
        case ETIMEDOUT: return "ETIMEDOUT - self explanatory";
        default: return "undocumented error!";
    }
}

char* senderrdesc(int err) {
    switch (err) {
        case EAGAIN: return "EAGAIN: queue is full so the call would have to block";
        case EBADF: return "EBADF: BUG!";
        case EINTR: return "EINTR: interrupted; try again";
        case EMSGSIZE: return "EMSGSIZE: the message is too big for the queue";
        case ETIMEDOUT: return "ETIMEDOUT - self explanatory";
        default: return "undocumented error!";
    }
}

char* unlinkerrdesc(int err) {
    switch (err) {
        case EACCES: return "EACCES: not permitted to delete this queue";
        case ENOENT: return "ENOENT: queue doesn't exist";
        case EINVAL: return "EINVAL: name is empty or does not start with a slash";
        case ENAMETOOLONG: return "ENAMETOOLONG - self explanatory";
        default: return "undocumented error!";
    }
}

int main(int argc, char* const* const argv) {
    mqd_t q = (mqd_t)-1;
    if (argc < 2) {
        usage();
    } else if (!strcmp(argv[1], "ls") && argc == 2) {
        DIR *dd = opendir("/dev/mqueue");
        if (dd == NULL) {
            printerr(errno, "oppening /dev/mqueue/", strerror, 1);
        }
        struct dirent *de;
        while ((de = readdir(dd)) != NULL) {
            if (strcmp(de->d_name, "..") && strcmp(de->d_name, ".")) {
                printf("/%s\n", de->d_name);
            }
        }
        closedir(dd);
    } else if ((!strcmp(argv[1], "rm") || !strcmp(argv[1], "unlink")) && argc > 2) {
        for (int i=2; i<argc; i++) {
            if (mq_unlink(argv[i])) {
                printerr(errno, "deleting", unlinkerrdesc, 1);
            }
        }
    } else if ((!strcmp(argv[1], "stat") || !strcmp(argv[1], "getattr")) && argc > 2 && argc%2 == 0) {
        for (int i=2; i<argc; i+=2) {
            mqd_t q = parseopts_open(argv[i], argv[i+1]);
            struct mq_attr attrs;
            if (mq_getattr(q, &attrs)) {
                perror("bug or undocumented error!");
                exit(1);
            }
            printf("maxmsg: %ld\nmsgsize: %ld\ncurmsgs: %ld\nflags: 0x%lx\n (nonblocking: %s)\n",
                (long)attrs.mq_maxmsg, (long)attrs.mq_msgsize, (long)attrs.mq_curmsgs,
                (long)attrs.mq_flags, attrs.mq_flags & O_NONBLOCK ? "yes" : "no"
            );
       }
       // there is not much point in exposing mq_setattr(), because
       // the only thing it can change is O_NONBLOCK
    } else if ((!strcmp(argv[1], "read") || !strcmp(argv[1], "receive"))
    && (argc == 4 || argc == 5)) {
        char buf[1024*1024];
        unsigned int prio;
        ssize_t len;
        q = parseopts_open(argv[2], argv[3]);
        if (argc == 4) {
            len = mq_receive(q, buf, 1024*1024, &prio);
        } else {
            struct timespec deadline = parse_timeout(argv[4]);
            len = mq_timedreceive(q, buf, 1024*1024, &prio, &deadline);
        }
        if (len == -1) {
            printerr(errno, "receiving", recverrdesc, 1);
        }
        printf("%2d ", prio);
        fwrite(&buf, len, 1, stdout);
        putchar('\n');
    } else if ((!strcmp(argv[1], "write") || !strcmp(argv[1], "send")) && argc == 6) {
        mqd_t q = parseopts_open(argv[2], argv[3]);
        if (mq_send(q, argv[5], strlen(argv[5]), atoi(argv[4]))) {
            printerr(errno, "sending", senderrdesc, 1);
        }
    } else if ((!strcmp(argv[1], "write") || !strcmp(argv[1], "send")) && argc == 7) {
        mqd_t q = parseopts_open(argv[2], argv[3]);
        struct timespec deadline = parse_timeout(argv[6]);
        if (mq_timedsend(q, argv[5], strlen(argv[5]), atoi(argv[4]), &deadline)) {
            printerr(errno, "sending", senderrdesc, 1);
        }
    } else {
        fprintf(stderr, "unknown operation or wrong number of arguments\n");
        usage();
    }

    if (q != (mqd_t)-1) {
        if (mq_close(q)) {
            perror("close queue");
            return 1;
        }
    }
    return 0;
}
