#include <assert.h>
#include <errno.h>
#include <linux/perf_event.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <syscall.h>
#include <unistd.h>

/*!
 * @brief Handle for operations with perf events.
 */
typedef struct x {
    int fd;
    unsigned char *perf_buffer;
    size_t perf_buffer_size;
} PerfEventHandle;

/*******************************************************************************
 * @brief Copy data from the perf ring buffer.
 *
 * Copies data from the ring buffer, handling the wrapping.
 *
 * @param dest Destination for the copied data, of len @p len.
 * @param src Perf ring buffer, of size @p src_size.
 * @param src_offset Offset in @p src where to copy from.
 *  Can be larger than src_size, is taken as modulo.
 * @param src_size Size of @p src ring buffer.
 * @param len Number of bytes to copy from (src+src_offset) to @p dest.
 ******************************************************************************/
static void
pe_memcpy(void *dest, const void *src, size_t src_offset, size_t src_size,
          size_t len) {
    assert(dest != NULL);
    assert(src != NULL);
    assert(src_size != 0);
    src_offset %= src_size;

    size_t end = src_offset + len;
    size_t fst_len = end > src_size ? src_size - src_offset : len;
    size_t sec_len = len - fst_len;
    (void)memcpy(dest, src + src_offset, fst_len);
    (void)memcpy(dest + fst_len, src, sec_len);
}

bool
pe_open(const struct perf_event_attr *attr, pid_t pid, int cpu, int group_fd,
        unsigned long flags, size_t num_pages, PerfEventHandle *handle) {
    if (attr == NULL || handle == NULL) {
        return false;
    }

    int fd = syscall(SYS_perf_event_open, attr, pid, cpu, group_fd, flags);
    if (fd < 0) {
        return false;
    }

    size_t map_size = getpagesize() * (1 + num_pages);

    void *buffer =
        mmap(NULL, map_size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    if (buffer == MAP_FAILED) {
        (void)close(fd);
        return false;
    }

    handle->fd = fd;
    handle->perf_buffer = buffer;
    handle->perf_buffer_size = map_size;
    return true;
}

bool
pe_open_cpu_sample(size_t cpu, size_t frequency, size_t num_pages,
                   PerfEventHandle *handle) {
    struct perf_event_attr attr = {0};
    attr.type = PERF_TYPE_SOFTWARE;
    attr.size = sizeof(attr);
    attr.config = PERF_COUNT_SW_TASK_CLOCK;
    attr.sample_freq = frequency;
    attr.freq = 1;

    attr.sample_type =
        PERF_SAMPLE_TID | PERF_SAMPLE_TIME | PERF_SAMPLE_IP | PERF_SAMPLE_CPU;
    attr.read_format = 0;

    attr.disabled = 1;
    attr.sample_id_all = 0;
    // Target 1sec poll roughly.
    attr.wakeup_events = frequency;

    return pe_open(&attr, -1, cpu, -1,
                   PERF_FLAG_FD_CLOEXEC | PERF_FLAG_FD_NO_GROUP, num_pages,
                   handle);
}

void
pe_close(PerfEventHandle *handle) {
    if (handle != NULL) {
        (void)munmap(handle->perf_buffer, handle->perf_buffer_size);
        (void)close(handle->fd);
    }
}

bool
pe_start(const PerfEventHandle *handle, bool do_reset) {
    if (handle == NULL) {
        return false;
    }
    return (do_reset && ioctl(handle->fd, PERF_EVENT_IOC_RESET, 0) == 0) &&
           ioctl(handle->fd, PERF_EVENT_IOC_ENABLE, 0) == 0;
}

bool
pe_stop(const PerfEventHandle *handle) {
    if (handle == NULL) {
        return false;
    }
    return ioctl(handle->fd, PERF_EVENT_IOC_DISABLE, 0) == 0;
}

size_t
pe_get_event(const PerfEventHandle *handle, unsigned char *dest, size_t n,
             bool peek_only) {
    struct perf_event_mmap_page *header = (void *)handle->perf_buffer;

    // The ring buffer begins at the next page.
    unsigned char *buffer = handle->perf_buffer + getpagesize();
    const size_t buffer_size = handle->perf_buffer_size - getpagesize();

    struct perf_event_header event_header = {0};
    atomic_thread_fence(memory_order_acquire);
    uint64_t tail = header->data_tail;
    uint64_t head = header->data_head;
    // Header does not fit -> no unread sample.
    // Both values are non-decreasing.
    if (tail + sizeof(event_header) > head)
        return 0;

    pe_memcpy(&event_header, buffer, tail, buffer_size, sizeof(event_header));
    assert(event_header.size >= sizeof(event_header));
    size_t event_size = event_header.size - sizeof(event_header);

    if (dest != NULL && n > 0) {
        // The event is only partially written. Can it even happen?
        if (tail + event_header.size > head)
            return 0;

        size_t n_to_copy = event_size < n ? event_size : n;
        pe_memcpy(dest, buffer, tail + sizeof(event_header), buffer_size,
                  n_to_copy);
    }

    if (!peek_only) {
        header->data_tail += event_header.size;
        atomic_thread_fence(memory_order_release);
    }
    // Report true size of the event.
    return event_size;
}
