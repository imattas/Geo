#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>

#if defined(_WIN32)
#include <direct.h>
#include <io.h>
#include <windows.h>
#define geo_environ _environ
#define geo_mkdir(path) _mkdir(path)
#define geo_rmdir _rmdir
#define geo_stat _stat
#define geo_getcwd _getcwd
#define geo_chdir _chdir
#define geo_setenv(name, value) _putenv_s((name), (value))
#define geo_unsetenv(name) _putenv_s((name), "")
#define geo_strtoi64(value, end, base) _strtoi64((value), (end), (base))
#define geo_strtou64(value, end, base) _strtoui64((value), (end), (base))
#define GEO_DIR_MODE _S_IFDIR
#define GEO_REG_MODE _S_IFREG
#define GEO_TYPE_MODE _S_IFMT
#else
#include <dirent.h>
#include <sys/time.h>
#include <sys/wait.h>
#include <unistd.h>
extern char **environ;
#define geo_environ environ
#define geo_mkdir(path) mkdir(path, 0777)
#define geo_rmdir rmdir
#define geo_stat stat
#define geo_getcwd getcwd
#define geo_chdir chdir
#define geo_setenv(name, value) setenv((name), (value), 1)
#define geo_unsetenv(name) unsetenv(name)
#define geo_strtoi64(value, end, base) strtoll((value), (end), (base))
#define geo_strtou64(value, end, base) strtoull((value), (end), (base))
#define GEO_DIR_MODE S_IFDIR
#define GEO_REG_MODE S_IFREG
#define GEO_TYPE_MODE S_IFMT
#endif

#define GEO_MAX_FILES 256

typedef struct GeoArrayHeader {
    uint64_t len;
    uint64_t cap;
    uint64_t elem_size;
    unsigned char data[];
} GeoArrayHeader;

static FILE *geo_files[GEO_MAX_FILES];
static int geo_argc = 0;
static char **geo_argv = NULL;
static uint64_t geo_random_state = 0x9e3779b97f4a7c15ULL;

int geo_main(void);
static int geo_is_path_separator(char ch);
static char *geo_join_child_path(const char *parent, const char *child);
int file_is_dir(const char *path);
int dir_exists(const char *path);
int create_dir_all(const char *path);
int copy_dir_all(const char *source, const char *dest);
int remove_dir_all(const char *path);
const char *path_join(const char *left, const char *right);

int main(int argc, char **argv) {
    geo_argc = argc;
    geo_argv = argv;
    return geo_main();
}

static FILE *geo_file_for_handle(int handle) {
    if (handle == 0) {
        return stdin;
    }
    if (handle == 1) {
        return stdout;
    }
    if (handle == 2) {
        return stderr;
    }
    if (handle < 3 || handle >= GEO_MAX_FILES) {
        return NULL;
    }
    return geo_files[handle];
}

static int geo_register_file(FILE *file) {
    if (file == NULL) {
        return -1;
    }
    for (int handle = 3; handle < GEO_MAX_FILES; handle++) {
        if (geo_files[handle] == NULL) {
            geo_files[handle] = file;
            return handle;
        }
    }
    fclose(file);
    return -1;
}

static GeoArrayHeader *geo_array_from_ptr(void *ptr) {
    return (GeoArrayHeader *)ptr;
}

static int geo_is_space(char ch) {
    return ch == ' ' || ch == '\t' || ch == '\n' || ch == '\r' || ch == '\f' || ch == '\v';
}

static const char *geo_string_slice(const char *start, size_t len) {
    char *out = (char *)malloc(len + 1);
    if (out == NULL) {
        return "";
    }
    memcpy(out, start, len);
    out[len] = '\0';
    return out;
}

static const char *geo_read_stream(FILE *file) {
    if (file == NULL) {
        return "";
    }
    long start = ftell(file);
    if (start < 0) {
        start = 0;
    }
    if (fseek(file, 0, SEEK_END) != 0) {
        return "";
    }
    long size = ftell(file);
    if (size < 0) {
        return "";
    }
    if (fseek(file, 0, SEEK_SET) != 0) {
        return "";
    }
    char *data = (char *)malloc((size_t)size + 1);
    if (data == NULL) {
        return "";
    }
    size_t read_count = fread(data, 1, (size_t)size, file);
    data[read_count] = '\0';
    fseek(file, start, SEEK_SET);
    return data;
}

int print(const char *value) {
    if (value == NULL) {
        return fputs("(null)", stdout) < 0 ? 1 : 0;
    }
    return fputs(value, stdout) < 0 ? 1 : 0;
}

int println(const char *value) {
    int status = print(value);
    if (putchar('\n') == EOF) {
        return 1;
    }
    return status;
}

int eprint(const char *value) {
    if (value == NULL) {
        return fputs("(null)", stderr) < 0 ? 1 : 0;
    }
    return fputs(value, stderr) < 0 ? 1 : 0;
}

const char *read_line(void) {
    size_t capacity = 128;
    size_t len = 0;
    char *buffer = (char *)malloc(capacity);
    if (buffer == NULL) {
        return "";
    }

    for (;;) {
        int ch = getchar();
        if (ch == EOF) {
            break;
        }
        if (len + 1 >= capacity) {
            size_t next_capacity = capacity * 2;
            char *next = (char *)realloc(buffer, next_capacity);
            if (next == NULL) {
                free(buffer);
                return "";
            }
            buffer = next;
            capacity = next_capacity;
        }
        buffer[len++] = (char)ch;
        if (ch == '\n') {
            break;
        }
    }

    buffer[len] = '\0';
    return buffer;
}

int file_open(const char *path) {
    if (path == NULL) {
        return -1;
    }
    FILE *file = fopen(path, "rb");
    return geo_register_file(file);
}

int file_open_write(const char *path) {
    if (path == NULL) {
        return -1;
    }
    FILE *file = fopen(path, "wb");
    return geo_register_file(file);
}

int file_open_append(const char *path) {
    if (path == NULL) {
        return -1;
    }
    FILE *file = fopen(path, "ab");
    return geo_register_file(file);
}

const char *read_file(const char *path) {
    if (path == NULL) {
        return "";
    }
    FILE *file = fopen(path, "rb");
    if (file == NULL) {
        return "";
    }
    const char *data = geo_read_stream(file);
    fclose(file);
    return data;
}

const char *read_file_or(const char *path, const char *default_value) {
    if (path == NULL) {
        return default_value == NULL ? "" : default_value;
    }
    FILE *file = fopen(path, "rb");
    if (file == NULL) {
        return default_value == NULL ? "" : default_value;
    }
    const char *data = geo_read_stream(file);
    fclose(file);
    return data;
}

int write_file(const char *path, const char *data) {
    if (path == NULL) {
        return 1;
    }
    FILE *file = fopen(path, "wb");
    if (file == NULL) {
        return 1;
    }
    if (data == NULL) {
        data = "";
    }
    size_t len = strlen(data);
    int status = fwrite(data, 1, len, file) == len ? 0 : 1;
    if (fclose(file) != 0) {
        status = 1;
    }
    return status;
}

int append_file(const char *path, const char *data) {
    if (path == NULL) {
        return 1;
    }
    FILE *file = fopen(path, "ab");
    if (file == NULL) {
        return 1;
    }
    if (data == NULL) {
        data = "";
    }
    size_t len = strlen(data);
    int status = fwrite(data, 1, len, file) == len ? 0 : 1;
    if (fclose(file) != 0) {
        status = 1;
    }
    return status;
}

int touch_file(const char *path) {
    if (path == NULL) {
        return 1;
    }
    FILE *file = fopen(path, "ab");
    if (file == NULL) {
        return 1;
    }
    return fclose(file) == 0 ? 0 : 1;
}

int truncate_file(const char *path, uint64_t size) {
    if (path == NULL) {
        return 1;
    }
#if defined(_WIN32)
    FILE *file = fopen(path, "r+b");
    if (file == NULL) {
        return 1;
    }
    int status = _chsize_s(_fileno(file), (__int64)size) == 0 ? 0 : 1;
    if (fclose(file) != 0) {
        status = 1;
    }
    return status;
#else
    return truncate(path, (off_t)size) == 0 ? 0 : 1;
#endif
}

int file_exists(const char *path) {
    if (path == NULL) {
        return 0;
    }
    FILE *file = fopen(path, "rb");
    if (file == NULL) {
        return 0;
    }
    fclose(file);
    return 1;
}

int file_is_file(const char *path) {
    if (path == NULL) {
        return 0;
    }
    struct geo_stat info;
    if (geo_stat(path, &info) != 0) {
        return 0;
    }
    return (info.st_mode & GEO_TYPE_MODE) == GEO_REG_MODE ? 1 : 0;
}

int file_is_empty(const char *path) {
    if (!file_is_file(path)) {
        return 0;
    }
    return file_size(path) == 0 ? 1 : 0;
}

int file_is_dir(const char *path) {
    if (path == NULL) {
        return 0;
    }
    struct geo_stat info;
    if (geo_stat(path, &info) != 0) {
        return 0;
    }
    return (info.st_mode & GEO_TYPE_MODE) == GEO_DIR_MODE ? 1 : 0;
}

int remove_file(const char *path) {
    if (path == NULL) {
        return 1;
    }
    return remove(path) == 0 ? 0 : 1;
}

uint64_t file_size(const char *path) {
    if (path == NULL) {
        return 0;
    }
    struct geo_stat info;
    if (geo_stat(path, &info) != 0) {
        return 0;
    }
    if (info.st_size < 0) {
        return 0;
    }
    return (uint64_t)info.st_size;
}

uint64_t file_modified_time(const char *path) {
    if (path == NULL) {
        return 0;
    }
    struct geo_stat info;
    if (geo_stat(path, &info) != 0) {
        return 0;
    }
    if (info.st_mtime < 0) {
        return 0;
    }
    return (uint64_t)info.st_mtime;
}

uint64_t file_accessed_time(const char *path) {
    if (path == NULL) {
        return 0;
    }
    struct geo_stat info;
    if (geo_stat(path, &info) != 0) {
        return 0;
    }
    if (info.st_atime < 0) {
        return 0;
    }
    return (uint64_t)info.st_atime;
}

uint64_t file_created_time(const char *path) {
    if (path == NULL) {
        return 0;
    }
    struct geo_stat info;
    if (geo_stat(path, &info) != 0) {
        return 0;
    }
    if (info.st_ctime < 0) {
        return 0;
    }
    return (uint64_t)info.st_ctime;
}

int copy_file(const char *source, const char *dest) {
    if (source == NULL || dest == NULL) {
        return 1;
    }
    FILE *input = fopen(source, "rb");
    if (input == NULL) {
        return 1;
    }
    FILE *output = fopen(dest, "wb");
    if (output == NULL) {
        fclose(input);
        return 1;
    }

    unsigned char buffer[8192];
    int status = 0;
    for (;;) {
        size_t read_count = fread(buffer, 1, sizeof(buffer), input);
        if (read_count > 0 && fwrite(buffer, 1, read_count, output) != read_count) {
            status = 1;
            break;
        }
        if (read_count < sizeof(buffer)) {
            if (ferror(input)) {
                status = 1;
            }
            break;
        }
    }

    if (fclose(output) != 0) {
        status = 1;
    }
    fclose(input);
    return status;
}

int rename_file(const char *source, const char *dest) {
    if (source == NULL || dest == NULL) {
        return 1;
    }
    return rename(source, dest) == 0 ? 0 : 1;
}

int copy_dir_all(const char *source, const char *dest) {
    if (source == NULL || dest == NULL || !dir_exists(source)) {
        return 1;
    }
    if (create_dir_all(dest) != 0) {
        return 1;
    }

#if defined(_WIN32)
    size_t source_len = strlen(source);
    int needs_separator = source_len > 0 && !geo_is_path_separator(source[source_len - 1]);
    char *pattern = (char *)malloc(source_len + (needs_separator ? 1 : 0) + 2);
    if (pattern == NULL) {
        return 1;
    }
    memcpy(pattern, source, source_len);
    size_t offset = source_len;
    if (needs_separator) {
        pattern[offset++] = (char)platform_path_separator();
    }
    pattern[offset++] = '*';
    pattern[offset] = '\0';

    WIN32_FIND_DATAA data;
    HANDLE handle = FindFirstFileA(pattern, &data);
    free(pattern);
    if (handle == INVALID_HANDLE_VALUE) {
        return 0;
    }
    do {
        if (strcmp(data.cFileName, ".") == 0 || strcmp(data.cFileName, "..") == 0) {
            continue;
        }
        char *source_child = geo_join_child_path(source, data.cFileName);
        char *dest_child = geo_join_child_path(dest, data.cFileName);
        if (source_child == NULL || dest_child == NULL) {
            free(source_child);
            free(dest_child);
            FindClose(handle);
            return 1;
        }
        int status = file_is_dir(source_child) ? copy_dir_all(source_child, dest_child)
                                              : copy_file(source_child, dest_child);
        free(source_child);
        free(dest_child);
        if (status != 0) {
            FindClose(handle);
            return 1;
        }
    } while (FindNextFileA(handle, &data));
    FindClose(handle);
#else
    DIR *dir = opendir(source);
    if (dir == NULL) {
        return 1;
    }
    for (;;) {
        struct dirent *entry = readdir(dir);
        if (entry == NULL) {
            break;
        }
        if (strcmp(entry->d_name, ".") == 0 || strcmp(entry->d_name, "..") == 0) {
            continue;
        }
        char *source_child = geo_join_child_path(source, entry->d_name);
        char *dest_child = geo_join_child_path(dest, entry->d_name);
        if (source_child == NULL || dest_child == NULL) {
            free(source_child);
            free(dest_child);
            closedir(dir);
            return 1;
        }
        int status = file_is_dir(source_child) ? copy_dir_all(source_child, dest_child)
                                              : copy_file(source_child, dest_child);
        free(source_child);
        free(dest_child);
        if (status != 0) {
            closedir(dir);
            return 1;
        }
    }
    closedir(dir);
#endif

    return 0;
}

int dir_exists(const char *path) {
    if (path == NULL) {
        return 0;
    }
    struct geo_stat info;
    if (geo_stat(path, &info) != 0) {
        return 0;
    }
    return (info.st_mode & GEO_DIR_MODE) ? 1 : 0;
}

uint64_t dir_entry_count(const char *path) {
    if (path == NULL) {
        return 0;
    }
    uint64_t count = 0;
#if defined(_WIN32)
    size_t path_len = strlen(path);
    int needs_separator = path_len > 0 && !geo_is_path_separator(path[path_len - 1]);
    char *pattern = (char *)malloc(path_len + (needs_separator ? 1 : 0) + 2);
    if (pattern == NULL) {
        return 0;
    }
    memcpy(pattern, path, path_len);
    size_t offset = path_len;
    if (needs_separator) {
        pattern[offset++] = (char)platform_path_separator();
    }
    pattern[offset++] = '*';
    pattern[offset] = '\0';

    WIN32_FIND_DATAA data;
    HANDLE handle = FindFirstFileA(pattern, &data);
    free(pattern);
    if (handle == INVALID_HANDLE_VALUE) {
        return 0;
    }
    do {
        if (strcmp(data.cFileName, ".") != 0 && strcmp(data.cFileName, "..") != 0) {
            count++;
        }
    } while (FindNextFileA(handle, &data));
    FindClose(handle);
#else
    DIR *dir = opendir(path);
    if (dir == NULL) {
        return 0;
    }
    for (;;) {
        struct dirent *entry = readdir(dir);
        if (entry == NULL) {
            break;
        }
        if (strcmp(entry->d_name, ".") != 0 && strcmp(entry->d_name, "..") != 0) {
            count++;
        }
    }
    closedir(dir);
#endif
    return count;
}

const char *dir_entry_name(const char *path, uint64_t index) {
    if (path == NULL) {
        return "";
    }
    uint64_t current = 0;
#if defined(_WIN32)
    size_t path_len = strlen(path);
    int needs_separator = path_len > 0 && !geo_is_path_separator(path[path_len - 1]);
    char *pattern = (char *)malloc(path_len + (needs_separator ? 1 : 0) + 2);
    if (pattern == NULL) {
        return "";
    }
    memcpy(pattern, path, path_len);
    size_t offset = path_len;
    if (needs_separator) {
        pattern[offset++] = (char)platform_path_separator();
    }
    pattern[offset++] = '*';
    pattern[offset] = '\0';

    WIN32_FIND_DATAA data;
    HANDLE handle = FindFirstFileA(pattern, &data);
    free(pattern);
    if (handle == INVALID_HANDLE_VALUE) {
        return "";
    }
    do {
        if (strcmp(data.cFileName, ".") == 0 || strcmp(data.cFileName, "..") == 0) {
            continue;
        }
        if (current == index) {
            const char *name = geo_string_slice(data.cFileName, strlen(data.cFileName));
            FindClose(handle);
            return name;
        }
        current++;
    } while (FindNextFileA(handle, &data));
    FindClose(handle);
#else
    DIR *dir = opendir(path);
    if (dir == NULL) {
        return "";
    }
    for (;;) {
        struct dirent *entry = readdir(dir);
        if (entry == NULL) {
            break;
        }
        if (strcmp(entry->d_name, ".") == 0 || strcmp(entry->d_name, "..") == 0) {
            continue;
        }
        if (current == index) {
            const char *name = geo_string_slice(entry->d_name, strlen(entry->d_name));
            closedir(dir);
            return name;
        }
        current++;
    }
    closedir(dir);
#endif
    return "";
}

const char *dir_entry_path(const char *path, uint64_t index) {
    if (path == NULL) {
        return "";
    }
    const char *name = dir_entry_name(path, index);
    if (name == NULL || name[0] == '\0') {
        return "";
    }
    return path_join(path, name);
}

int create_dir(const char *path) {
    if (path == NULL) {
        return 1;
    }
    return geo_mkdir(path) == 0 ? 0 : 1;
}

int create_dir_all(const char *path) {
    if (path == NULL || path[0] == '\0') {
        return 1;
    }
    if (dir_exists(path)) {
        return 0;
    }

    size_t len = strlen(path);
    char *buffer = (char *)malloc(len + 1);
    if (buffer == NULL) {
        return 1;
    }
    memcpy(buffer, path, len + 1);

    for (size_t i = 0; i < len; i++) {
        if (!geo_is_path_separator(buffer[i])) {
            continue;
        }
        if (i == 0 || (i == 2 && buffer[1] == ':')) {
            continue;
        }

        char saved = buffer[i];
        buffer[i] = '\0';
        if (buffer[0] != '\0' && !dir_exists(buffer)) {
            if (geo_mkdir(buffer) != 0 && !dir_exists(buffer)) {
                free(buffer);
                return 1;
            }
        }
        buffer[i] = saved;
    }

    if (!dir_exists(buffer)) {
        if (geo_mkdir(buffer) != 0 && !dir_exists(buffer)) {
            free(buffer);
            return 1;
        }
    }
    free(buffer);
    return 0;
}

int remove_dir(const char *path) {
    if (path == NULL) {
        return 1;
    }
    return geo_rmdir(path) == 0 ? 0 : 1;
}

static char *geo_join_child_path(const char *parent, const char *child) {
    size_t parent_len = strlen(parent);
    size_t child_len = strlen(child);
    int needs_separator = parent_len > 0 && !geo_is_path_separator(parent[parent_len - 1]);
    char *out = (char *)malloc(parent_len + (needs_separator ? 1 : 0) + child_len + 1);
    if (out == NULL) {
        return NULL;
    }
    memcpy(out, parent, parent_len);
    size_t offset = parent_len;
    if (needs_separator) {
        out[offset++] = (char)platform_path_separator();
    }
    memcpy(out + offset, child, child_len);
    out[offset + child_len] = '\0';
    return out;
}

static int geo_remove_dir_child(const char *path) {
    if (file_is_dir(path)) {
        return remove_dir_all(path);
    }
    return remove(path) == 0 ? 0 : 1;
}

int remove_dir_all(const char *path) {
    if (path == NULL) {
        return 1;
    }
    if (!dir_exists(path)) {
        return 1;
    }

#if defined(_WIN32)
    size_t path_len = strlen(path);
    int needs_separator = path_len > 0 && !geo_is_path_separator(path[path_len - 1]);
    char *pattern = (char *)malloc(path_len + (needs_separator ? 1 : 0) + 2);
    if (pattern == NULL) {
        return 1;
    }
    memcpy(pattern, path, path_len);
    size_t offset = path_len;
    if (needs_separator) {
        pattern[offset++] = (char)platform_path_separator();
    }
    pattern[offset++] = '*';
    pattern[offset] = '\0';

    WIN32_FIND_DATAA data;
    HANDLE handle = FindFirstFileA(pattern, &data);
    free(pattern);
    if (handle != INVALID_HANDLE_VALUE) {
        do {
            if (strcmp(data.cFileName, ".") == 0 || strcmp(data.cFileName, "..") == 0) {
                continue;
            }
            char *child = geo_join_child_path(path, data.cFileName);
            if (child == NULL) {
                FindClose(handle);
                return 1;
            }
            int status = geo_remove_dir_child(child);
            free(child);
            if (status != 0) {
                FindClose(handle);
                return 1;
            }
        } while (FindNextFileA(handle, &data));
        FindClose(handle);
    }
#else
    DIR *dir = opendir(path);
    if (dir == NULL) {
        return 1;
    }
    for (;;) {
        struct dirent *entry = readdir(dir);
        if (entry == NULL) {
            break;
        }
        if (strcmp(entry->d_name, ".") == 0 || strcmp(entry->d_name, "..") == 0) {
            continue;
        }
        char *child = geo_join_child_path(path, entry->d_name);
        if (child == NULL) {
            closedir(dir);
            return 1;
        }
        int status = geo_remove_dir_child(child);
        free(child);
        if (status != 0) {
            closedir(dir);
            return 1;
        }
    }
    closedir(dir);
#endif

    return geo_rmdir(path) == 0 ? 0 : 1;
}

const char *file_read(int handle) {
    FILE *file = geo_file_for_handle(handle);
    return geo_read_stream(file);
}

int file_write(int handle, const char *data) {
    FILE *file = geo_file_for_handle(handle);
    if (file == NULL) {
        return 1;
    }
    if (data == NULL) {
        data = "";
    }
    size_t len = strlen(data);
    return fwrite(data, 1, len, file) == len ? 0 : 1;
}

int file_close(int handle) {
    if (handle < 3 || handle >= GEO_MAX_FILES || geo_files[handle] == NULL) {
        return 0;
    }
    fclose(geo_files[handle]);
    geo_files[handle] = NULL;
    return 0;
}

void *alloc(uint64_t size) {
    return malloc((size_t)size);
}

void *alloc_zeroed(uint64_t size) {
    if (size > (uint64_t)SIZE_MAX) {
        return NULL;
    }
    return calloc(1, (size_t)size);
}

void *alloc_array(uint64_t element_size, uint64_t count) {
    if (element_size != 0 && count > UINT64_MAX / element_size) {
        return NULL;
    }
    uint64_t total = element_size * count;
    if (total > (uint64_t)SIZE_MAX) {
        return NULL;
    }
    return calloc((size_t)count, (size_t)element_size);
}

void *alloc_copy(const void *src, uint64_t len) {
    if (src == NULL && len != 0) {
        return NULL;
    }
    if (len > (uint64_t)SIZE_MAX) {
        return NULL;
    }
    void *out = malloc((size_t)len);
    if (out == NULL) {
        return NULL;
    }
    if (len != 0) {
        memcpy(out, src, (size_t)len);
    }
    return out;
}

void *realloc_array(void *ptr, uint64_t element_size, uint64_t count) {
    if (element_size != 0 && count > UINT64_MAX / element_size) {
        return NULL;
    }
    uint64_t total = element_size * count;
    if (total > (uint64_t)SIZE_MAX) {
        return NULL;
    }
    return realloc(ptr, (size_t)total);
}

uint64_t align_up(uint64_t value, uint64_t alignment) {
    if (alignment == 0) {
        return value;
    }
    uint64_t remainder = value % alignment;
    if (remainder == 0) {
        return value;
    }
    uint64_t delta = alignment - remainder;
    if (value > UINT64_MAX - delta) {
        return 0;
    }
    return value + delta;
}

uint64_t align_down(uint64_t value, uint64_t alignment) {
    if (alignment == 0) {
        return value;
    }
    return value - (value % alignment);
}

int is_aligned(uint64_t value, uint64_t alignment) {
    if (alignment == 0) {
        return 0;
    }
    return value % alignment == 0 ? 1 : 0;
}

int free_geo(void *ptr) {
    free(ptr);
    return 0;
}

void *realloc_geo(void *ptr, uint64_t size) {
    return realloc(ptr, (size_t)size);
}

int copy(void *dst, const void *src, uint64_t len) {
    memcpy(dst, src, (size_t)len);
    return 0;
}

int zero(void *dst, uint64_t len) {
    memset(dst, 0, (size_t)len);
    return 0;
}

int mem_compare(const void *left, const void *right, uint64_t len) {
    if (len == 0) {
        return 0;
    }
    if (left == NULL && right == NULL) {
        return 0;
    }
    if (left == NULL) {
        return -1;
    }
    if (right == NULL) {
        return 1;
    }
    int result = memcmp(left, right, (size_t)len);
    if (result < 0) {
        return -1;
    }
    if (result > 0) {
        return 1;
    }
    return 0;
}

int mem_equal(const void *left, const void *right, uint64_t len) {
    if (len == 0) {
        return 1;
    }
    if (left == NULL || right == NULL) {
        return left == right;
    }
    return memcmp(left, right, (size_t)len) == 0;
}

int mem_is_zero(const void *ptr, uint64_t len) {
    if (len == 0) {
        return 1;
    }
    if (ptr == NULL) {
        return 0;
    }
    const uint8_t *bytes = (const uint8_t *)ptr;
    for (uint64_t i = 0; i < len; i++) {
        if (bytes[i] != 0) {
            return 0;
        }
    }
    return 1;
}

int mem_swap(void *left, void *right, uint64_t len) {
    if (len == 0 || left == right) {
        return 0;
    }
    if (left == NULL || right == NULL) {
        return 1;
    }
    uint8_t *left_bytes = (uint8_t *)left;
    uint8_t *right_bytes = (uint8_t *)right;
    for (uint64_t i = 0; i < len; i++) {
        uint8_t tmp = left_bytes[i];
        left_bytes[i] = right_bytes[i];
        right_bytes[i] = tmp;
    }
    return 0;
}

int mem_reverse(void *ptr, uint64_t len) {
    if (len == 0) {
        return 0;
    }
    if (ptr == NULL) {
        return 1;
    }
    uint8_t *bytes = (uint8_t *)ptr;
    uint64_t left = 0;
    uint64_t right = len - 1;
    while (left < right) {
        uint8_t tmp = bytes[left];
        bytes[left] = bytes[right];
        bytes[right] = tmp;
        left++;
        right--;
    }
    return 0;
}

int mem_fill(void *dst, uint64_t len, uint8_t value) {
    if (len == 0) {
        return 0;
    }
    if (dst == NULL) {
        return 1;
    }
    memset(dst, value, (size_t)len);
    return 0;
}

uint64_t mem_replace_byte(void *ptr, uint64_t len, uint8_t old_value, uint8_t new_value) {
    if (ptr == NULL || len == 0) {
        return 0;
    }
    uint8_t *bytes = (uint8_t *)ptr;
    uint64_t count = 0;
    for (uint64_t i = 0; i < len; i++) {
        if (bytes[i] == old_value) {
            bytes[i] = new_value;
            count++;
        }
    }
    return count;
}

uint64_t mem_replace_pattern(
        void *ptr,
        uint64_t len,
        const void *pattern,
        uint64_t pattern_len,
        const void *replacement,
        uint64_t replacement_len) {
    if (ptr == NULL || pattern == NULL || replacement == NULL || len == 0 || pattern_len == 0 ||
        pattern_len != replacement_len || pattern_len > len) {
        return 0;
    }
    uint8_t *bytes = (uint8_t *)ptr;
    uint64_t count = 0;
    uint64_t limit = len - pattern_len;
    uint64_t i = 0;
    while (i <= limit) {
        if (memcmp(bytes + (size_t)i, pattern, (size_t)pattern_len) == 0) {
            memcpy(bytes + (size_t)i, replacement, (size_t)replacement_len);
            count++;
            i += pattern_len;
        } else {
            i++;
        }
    }
    return count;
}

int mem_xor_byte(void *ptr, uint64_t len, uint8_t mask) {
    if (len == 0) {
        return 0;
    }
    if (ptr == NULL) {
        return 1;
    }
    uint8_t *bytes = (uint8_t *)ptr;
    for (uint64_t i = 0; i < len; i++) {
        bytes[i] ^= mask;
    }
    return 0;
}

int mem_repeat_pattern(void *dst, uint64_t len, const void *pattern, uint64_t pattern_len) {
    if (len == 0) {
        return 0;
    }
    if (dst == NULL || pattern == NULL || pattern_len == 0) {
        return 1;
    }
    uint8_t *bytes = (uint8_t *)dst;
    const uint8_t *pattern_bytes = (const uint8_t *)pattern;
    for (uint64_t i = 0; i < len; i++) {
        bytes[i] = pattern_bytes[i % pattern_len];
    }
    return 0;
}

int mem_rotate_left(void *ptr, uint64_t len, uint64_t amount) {
    if (len == 0) {
        return 0;
    }
    if (ptr == NULL) {
        return 1;
    }
    amount %= len;
    if (amount == 0) {
        return 0;
    }
    uint8_t *bytes = (uint8_t *)ptr;
    uint64_t left = 0;
    uint64_t right = amount - 1;
    while (left < right) {
        uint8_t tmp = bytes[left];
        bytes[left] = bytes[right];
        bytes[right] = tmp;
        left++;
        right--;
    }
    left = amount;
    right = len - 1;
    while (left < right) {
        uint8_t tmp = bytes[left];
        bytes[left] = bytes[right];
        bytes[right] = tmp;
        left++;
        right--;
    }
    left = 0;
    right = len - 1;
    while (left < right) {
        uint8_t tmp = bytes[left];
        bytes[left] = bytes[right];
        bytes[right] = tmp;
        left++;
        right--;
    }
    return 0;
}

int mem_rotate_right(void *ptr, uint64_t len, uint64_t amount) {
    if (len == 0) {
        return 0;
    }
    amount %= len;
    if (amount == 0) {
        return mem_rotate_left(ptr, len, 0);
    }
    return mem_rotate_left(ptr, len, len - amount);
}

int mem_move(void *dst, const void *src, uint64_t len) {
    if (len == 0) {
        return 0;
    }
    if (dst == NULL || src == NULL) {
        return 1;
    }
    memmove(dst, src, (size_t)len);
    return 0;
}

int mem_copy(void *dst, const void *src, uint64_t len) {
    if (len == 0) {
        return 0;
    }
    if (dst == NULL || src == NULL) {
        return 1;
    }
    memcpy(dst, src, (size_t)len);
    return 0;
}

int mem_zero(void *dst, uint64_t len) {
    if (len == 0) {
        return 0;
    }
    if (dst == NULL) {
        return 1;
    }
    memset(dst, 0, (size_t)len);
    return 0;
}

int mem_find(const void *ptr, uint64_t len, uint8_t value) {
    if (ptr == NULL || len == 0) {
        return -1;
    }
    const uint8_t *bytes = (const uint8_t *)ptr;
    for (uint64_t i = 0; i < len; i++) {
        if (bytes[i] == value) {
            if (i > (uint64_t)INT32_MAX) {
                return -1;
            }
            return (int)i;
        }
    }
    return -1;
}

int mem_find_pattern(const void *ptr, uint64_t len, const void *pattern, uint64_t pattern_len) {
    if (pattern_len == 0) {
        return 0;
    }
    if (ptr == NULL || pattern == NULL || pattern_len > len) {
        return -1;
    }
    const uint8_t *bytes = (const uint8_t *)ptr;
    uint64_t limit = len - pattern_len;
    for (uint64_t i = 0; i <= limit; i++) {
        if (memcmp(bytes + (size_t)i, pattern, (size_t)pattern_len) == 0) {
            if (i > (uint64_t)INT32_MAX) {
                return -1;
            }
            return (int)i;
        }
    }
    return -1;
}

int mem_last_find(const void *ptr, uint64_t len, uint8_t value) {
    if (ptr == NULL || len == 0) {
        return -1;
    }
    const uint8_t *bytes = (const uint8_t *)ptr;
    uint64_t i = len;
    while (i > 0) {
        i--;
        if (bytes[i] == value) {
            if (i > (uint64_t)INT32_MAX) {
                return -1;
            }
            return (int)i;
        }
    }
    return -1;
}

int mem_last_find_pattern(const void *ptr, uint64_t len, const void *pattern, uint64_t pattern_len) {
    if (pattern_len == 0) {
        if (len > (uint64_t)INT32_MAX) {
            return -1;
        }
        return (int)len;
    }
    if (ptr == NULL || pattern == NULL || pattern_len > len) {
        return -1;
    }
    const uint8_t *bytes = (const uint8_t *)ptr;
    uint64_t i = len - pattern_len + 1;
    while (i > 0) {
        i--;
        if (memcmp(bytes + (size_t)i, pattern, (size_t)pattern_len) == 0) {
            if (i > (uint64_t)INT32_MAX) {
                return -1;
            }
            return (int)i;
        }
    }
    return -1;
}

uint64_t mem_count(const void *ptr, uint64_t len, uint8_t value) {
    if (ptr == NULL || len == 0) {
        return 0;
    }
    const uint8_t *bytes = (const uint8_t *)ptr;
    uint64_t count = 0;
    for (uint64_t i = 0; i < len; i++) {
        if (bytes[i] == value) {
            count++;
        }
    }
    return count;
}

uint64_t mem_count_pattern(const void *ptr, uint64_t len, const void *pattern, uint64_t pattern_len) {
    if (ptr == NULL || pattern == NULL || pattern_len == 0 || pattern_len > len) {
        return 0;
    }
    const uint8_t *bytes = (const uint8_t *)ptr;
    uint64_t count = 0;
    uint64_t limit = len - pattern_len;
    for (uint64_t i = 0; i <= limit; i++) {
        if (memcmp(bytes + (size_t)i, pattern, (size_t)pattern_len) == 0) {
            count++;
        }
    }
    return count;
}

uint64_t mem_split_count(const void *ptr, uint64_t len, uint8_t delimiter) {
    if (ptr == NULL || len == 0) {
        return 0;
    }
    return mem_count(ptr, len, delimiter) + 1;
}

uint64_t mem_split_count_pattern(
        const void *ptr,
        uint64_t len,
        const void *delimiter,
        uint64_t delimiter_len) {
    if (ptr == NULL || delimiter == NULL || len == 0 || delimiter_len == 0 || delimiter_len > len) {
        return 0;
    }
    const uint8_t *bytes = (const uint8_t *)ptr;
    uint64_t fields = 1;
    uint64_t limit = len - delimiter_len;
    uint64_t i = 0;
    while (i <= limit) {
        if (memcmp(bytes + (size_t)i, delimiter, (size_t)delimiter_len) == 0) {
            fields++;
            i += delimiter_len;
        } else {
            i++;
        }
    }
    return fields;
}

int mem_split_field_start(const void *ptr, uint64_t len, uint8_t delimiter, uint64_t field_index) {
    if (ptr == NULL || len == 0) {
        return -1;
    }
    const uint8_t *bytes = (const uint8_t *)ptr;
    uint64_t field = 0;
    uint64_t start = 0;
    for (uint64_t i = 0; i < len; i++) {
        if (bytes[i] == delimiter) {
            if (field == field_index) {
                return start > (uint64_t)INT32_MAX ? -1 : (int)start;
            }
            field++;
            start = i + 1;
        }
    }
    if (field == field_index) {
        return start > (uint64_t)INT32_MAX ? -1 : (int)start;
    }
    return -1;
}

uint64_t mem_split_field_len(const void *ptr, uint64_t len, uint8_t delimiter, uint64_t field_index) {
    int start_value = mem_split_field_start(ptr, len, delimiter, field_index);
    if (start_value < 0) {
        return 0;
    }
    uint64_t start = (uint64_t)start_value;
    const uint8_t *bytes = (const uint8_t *)ptr;
    uint64_t end = start;
    while (end < len && bytes[end] != delimiter) {
        end++;
    }
    return end - start;
}

int mem_split_field_start_pattern(
        const void *ptr,
        uint64_t len,
        const void *delimiter,
        uint64_t delimiter_len,
        uint64_t field_index) {
    if (ptr == NULL || delimiter == NULL || len == 0 || delimiter_len == 0 || delimiter_len > len) {
        return -1;
    }
    const uint8_t *bytes = (const uint8_t *)ptr;
    uint64_t field = 0;
    uint64_t start = 0;
    uint64_t limit = len - delimiter_len;
    uint64_t i = 0;
    while (i <= limit) {
        if (memcmp(bytes + (size_t)i, delimiter, (size_t)delimiter_len) == 0) {
            if (field == field_index) {
                return start > (uint64_t)INT32_MAX ? -1 : (int)start;
            }
            field++;
            i += delimiter_len;
            start = i;
        } else {
            i++;
        }
    }
    if (field == field_index) {
        return start > (uint64_t)INT32_MAX ? -1 : (int)start;
    }
    return -1;
}

uint64_t mem_split_field_len_pattern(
        const void *ptr,
        uint64_t len,
        const void *delimiter,
        uint64_t delimiter_len,
        uint64_t field_index) {
    int start_value = mem_split_field_start_pattern(ptr, len, delimiter, delimiter_len, field_index);
    if (start_value < 0) {
        return 0;
    }
    uint64_t start = (uint64_t)start_value;
    if (start >= len) {
        return 0;
    }
    int next = mem_find_pattern((const uint8_t *)ptr + (size_t)start, len - start, delimiter, delimiter_len);
    if (next < 0) {
        return len - start;
    }
    return (uint64_t)next;
}

uint64_t mem_line_count(const void *ptr, uint64_t len) {
    if (ptr == NULL || len == 0) {
        return 0;
    }
    const uint8_t *bytes = (const uint8_t *)ptr;
    uint64_t count = 0;
    for (uint64_t i = 0; i < len; i++) {
        if (bytes[i] == '\n') {
            count++;
        }
    }
    if (bytes[len - 1] != '\n') {
        count++;
    }
    return count;
}

int mem_line_start(const void *ptr, uint64_t len, uint64_t line_index) {
    if (ptr == NULL || len == 0) {
        return -1;
    }
    const uint8_t *bytes = (const uint8_t *)ptr;
    uint64_t line = 0;
    uint64_t start = 0;
    if (line_index == 0) {
        return 0;
    }
    for (uint64_t i = 0; i < len; i++) {
        if (bytes[i] == '\n') {
            line++;
            start = i + 1;
            if (line == line_index && start < len) {
                return start > (uint64_t)INT32_MAX ? -1 : (int)start;
            }
        }
    }
    return -1;
}

uint64_t mem_line_len(const void *ptr, uint64_t len, uint64_t line_index) {
    int start_value = mem_line_start(ptr, len, line_index);
    if (start_value < 0) {
        return 0;
    }
    uint64_t start = (uint64_t)start_value;
    const uint8_t *bytes = (const uint8_t *)ptr;
    uint64_t end = start;
    while (end < len && bytes[end] != '\n') {
        end++;
    }
    if (end > start && bytes[end - 1] == '\r') {
        end--;
    }
    return end - start;
}

int mem_line_index_at(const void *ptr, uint64_t len, uint64_t offset) {
    if (ptr == NULL || len == 0 || offset >= len) {
        return -1;
    }
    const uint8_t *bytes = (const uint8_t *)ptr;
    uint64_t line = 0;
    for (uint64_t i = 0; i < offset; i++) {
        if (bytes[i] == '\n') {
            line++;
        }
    }
    return line > (uint64_t)INT32_MAX ? -1 : (int)line;
}

int mem_column_at(const void *ptr, uint64_t len, uint64_t offset) {
    if (ptr == NULL || len == 0 || offset >= len) {
        return -1;
    }
    const uint8_t *bytes = (const uint8_t *)ptr;
    uint64_t start = 0;
    for (uint64_t i = 0; i < offset; i++) {
        if (bytes[i] == '\n') {
            start = i + 1;
        }
    }
    uint64_t column = offset - start;
    return column > (uint64_t)INT32_MAX ? -1 : (int)column;
}

int mem_offset_at_line_column(const void *ptr, uint64_t len, uint64_t line_index, uint64_t column) {
    int start_value = mem_line_start(ptr, len, line_index);
    if (start_value < 0) {
        return -1;
    }
    uint64_t line_len = mem_line_len(ptr, len, line_index);
    if (column > line_len) {
        return -1;
    }
    uint64_t offset = (uint64_t)start_value + column;
    return offset > (uint64_t)INT32_MAX ? -1 : (int)offset;
}

uint64_t mem_hash_seed(const void *ptr, uint64_t len, uint64_t seed) {
    const uint64_t prime = 1099511628211ULL;
    if (len == 0) {
        return seed;
    }
    if (ptr == NULL) {
        return 0;
    }
    const uint8_t *bytes = (const uint8_t *)ptr;
    uint64_t hash = seed;
    for (uint64_t i = 0; i < len; i++) {
        hash ^= bytes[i];
        hash *= prime;
    }
    return hash;
}

uint64_t mem_hash(const void *ptr, uint64_t len) {
    const uint64_t offset_basis = 14695981039346656037ULL;
    return mem_hash_seed(ptr, len, offset_basis);
}

int mem_contains(const void *ptr, uint64_t len, uint8_t value) {
    return mem_find(ptr, len, value) >= 0 ? 1 : 0;
}

int mem_all(const void *ptr, uint64_t len, uint8_t value) {
    if (len == 0) {
        return 1;
    }
    if (ptr == NULL) {
        return 0;
    }
    const uint8_t *bytes = (const uint8_t *)ptr;
    for (uint64_t i = 0; i < len; i++) {
        if (bytes[i] != value) {
            return 0;
        }
    }
    return 1;
}

int mem_any(const void *ptr, uint64_t len, uint8_t value) {
    return mem_contains(ptr, len, value);
}

uint64_t mem_leading_count(const void *ptr, uint64_t len, uint8_t value) {
    if (ptr == NULL || len == 0) {
        return 0;
    }
    const uint8_t *bytes = (const uint8_t *)ptr;
    uint64_t count = 0;
    while (count < len && bytes[count] == value) {
        count++;
    }
    return count;
}

uint64_t mem_trailing_count(const void *ptr, uint64_t len, uint8_t value) {
    if (ptr == NULL || len == 0) {
        return 0;
    }
    const uint8_t *bytes = (const uint8_t *)ptr;
    uint64_t count = 0;
    uint64_t index = len;
    while (index > 0) {
        index--;
        if (bytes[index] != value) {
            break;
        }
        count++;
    }
    return count;
}

uint64_t mem_trimmed_len(const void *ptr, uint64_t len, uint8_t value) {
    if (ptr == NULL || len == 0) {
        return 0;
    }
    uint64_t leading = mem_leading_count(ptr, len, value);
    if (leading == len) {
        return 0;
    }
    uint64_t trailing = mem_trailing_count(ptr, len, value);
    return len - leading - trailing;
}

int mem_starts_with(const void *ptr, uint64_t len, const void *prefix, uint64_t prefix_len) {
    if (prefix_len == 0) {
        return 1;
    }
    if (prefix_len > len || ptr == NULL || prefix == NULL) {
        return 0;
    }
    return memcmp(ptr, prefix, (size_t)prefix_len) == 0;
}

int mem_ends_with(const void *ptr, uint64_t len, const void *suffix, uint64_t suffix_len) {
    if (suffix_len == 0) {
        return 1;
    }
    if (suffix_len > len || ptr == NULL || suffix == NULL) {
        return 0;
    }
    const uint8_t *bytes = (const uint8_t *)ptr;
    return memcmp(bytes + (size_t)(len - suffix_len), suffix, (size_t)suffix_len) == 0;
}

int random_seed(uint64_t seed) {
    geo_random_state = seed == 0 ? 0x9e3779b97f4a7c15ULL : seed;
    return 0;
}

uint64_t random_usize(void) {
    geo_random_state = geo_random_state * 6364136223846793005ULL + 1442695040888963407ULL;
    return geo_random_state;
}

uint64_t random_range(uint64_t max) {
    if (max == 0) {
        return 0;
    }
    return random_usize() % max;
}

uint64_t random_range_inclusive(uint64_t max) {
    if (max == UINT64_MAX) {
        return random_usize();
    }
    return random_range(max + 1ULL);
}

int random_bool(void) {
    return (random_usize() & 1ULL) == 1ULL ? 1 : 0;
}

int64_t random_int_range(int64_t min, int64_t max) {
    if (min >= max) {
        return min;
    }
    uint64_t width = (uint64_t)(max - min);
    return min + (int64_t)random_range(width);
}

uint64_t hash_string(const char *value) {
    uint64_t hash = 1469598103934665603ULL;
    if (value == NULL) {
        return hash;
    }
    while (*value != '\0') {
        hash ^= (unsigned char)*value;
        hash *= 1099511628211ULL;
        value++;
    }
    return hash;
}

uint64_t hash_usize(uint64_t value) {
    uint64_t hash = 1469598103934665603ULL;
    for (int i = 0; i < 8; i++) {
        hash ^= (unsigned char)(value & 0xffu);
        hash *= 1099511628211ULL;
        value >>= 8;
    }
    return hash;
}

static uint64_t geo_hash_bytes_with_seed(const void *ptr, uint64_t len, uint64_t seed) {
    uint64_t hash = seed;
    if (ptr == NULL) {
        return hash;
    }
    const uint8_t *bytes = (const uint8_t *)ptr;
    for (uint64_t i = 0; i < len; i++) {
        hash ^= bytes[i];
        hash *= 1099511628211ULL;
    }
    return hash;
}

uint64_t hash_bytes(const void *ptr, uint64_t len) {
    return geo_hash_bytes_with_seed(ptr, len, 1469598103934665603ULL);
}

uint64_t hash_bytes_seed(const void *ptr, uint64_t len, uint64_t seed) {
    return geo_hash_bytes_with_seed(ptr, len, seed);
}

uint64_t hash_combine(uint64_t left, uint64_t right) {
    uint64_t hash = hash_usize(left);
    for (int i = 0; i < 8; i++) {
        hash ^= (unsigned char)(right & 0xffu);
        hash *= 1099511628211ULL;
        right >>= 8;
    }
    return hash;
}

uint64_t usize_popcount(uint64_t value) {
    uint64_t count = 0;
    while (value != 0) {
        count += value & 1u;
        value >>= 1;
    }
    return count;
}

uint64_t usize_count_ones(uint64_t value) {
    return usize_popcount(value);
}

int usize_parity(uint64_t value) {
    return (usize_popcount(value) & 1u) != 0;
}

uint64_t usize_count_zeros(uint64_t value) {
    return 64 - usize_popcount(value);
}

uint64_t int_popcount(int64_t value) {
    return usize_popcount((uint64_t)value);
}

uint64_t int_count_ones(int64_t value) {
    return int_popcount(value);
}

int int_parity(int64_t value) {
    return usize_parity((uint64_t)value);
}

uint64_t int_count_zeros(int64_t value) {
    return usize_count_zeros((uint64_t)value);
}

uint64_t usize_leading_zeros(uint64_t value) {
    if (value == 0) {
        return 64;
    }
    uint64_t count = 0;
    for (int bit = 63; bit >= 0; bit--) {
        if (((value >> bit) & 1u) != 0) {
            break;
        }
        count++;
    }
    return count;
}

uint64_t int_leading_zeros(int64_t value) {
    return usize_leading_zeros((uint64_t)value);
}

uint64_t usize_leading_ones(uint64_t value) {
    return usize_leading_zeros(~value);
}

uint64_t int_leading_ones(int64_t value) {
    return usize_leading_ones((uint64_t)value);
}

uint64_t usize_trailing_zeros(uint64_t value) {
    if (value == 0) {
        return 64;
    }
    uint64_t count = 0;
    while ((value & 1u) == 0) {
        count++;
        value >>= 1;
    }
    return count;
}

uint64_t int_trailing_zeros(int64_t value) {
    return usize_trailing_zeros((uint64_t)value);
}

uint64_t usize_trailing_ones(uint64_t value) {
    return usize_trailing_zeros(~value);
}

uint64_t int_trailing_ones(int64_t value) {
    return usize_trailing_ones((uint64_t)value);
}

uint64_t usize_reverse_bits(uint64_t value) {
    uint64_t out = 0;
    for (int i = 0; i < 64; i++) {
        out <<= 1;
        out |= value & 1u;
        value >>= 1;
    }
    return out;
}

int64_t int_reverse_bits(int64_t value) {
    return (int64_t)usize_reverse_bits((uint64_t)value);
}

uint64_t usize_swap_bytes(uint64_t value) {
    uint64_t out = 0;
    for (int i = 0; i < 8; i++) {
        out <<= 8;
        out |= value & 0xffu;
        value >>= 8;
    }
    return out;
}

int64_t int_swap_bytes(int64_t value) {
    return (int64_t)usize_swap_bytes((uint64_t)value);
}

static int geo_is_little_endian(void) {
    const uint16_t value = 1;
    return *((const uint8_t *)&value) == 1;
}

uint64_t usize_from_be(uint64_t value) {
    return geo_is_little_endian() ? usize_swap_bytes(value) : value;
}

uint64_t usize_from_le(uint64_t value) {
    return geo_is_little_endian() ? value : usize_swap_bytes(value);
}

uint64_t usize_to_be(uint64_t value) {
    return usize_from_be(value);
}

uint64_t usize_to_le(uint64_t value) {
    return usize_from_le(value);
}

int64_t int_from_be(int64_t value) {
    return (int64_t)usize_from_be((uint64_t)value);
}

int64_t int_from_le(int64_t value) {
    return (int64_t)usize_from_le((uint64_t)value);
}

int64_t int_to_be(int64_t value) {
    return int_from_be(value);
}

int64_t int_to_le(int64_t value) {
    return int_from_le(value);
}

uint64_t usize_bit_width(uint64_t value) {
    return value == 0 ? 0 : 64 - usize_leading_zeros(value);
}

uint64_t int_bit_width(int64_t value) {
    return usize_bit_width((uint64_t)value);
}

uint64_t usize_lowest_one(uint64_t value) {
    return value & (0ULL - value);
}

int64_t int_lowest_one(int64_t value) {
    return (int64_t)usize_lowest_one((uint64_t)value);
}

uint64_t usize_highest_one(uint64_t value) {
    uint64_t width = usize_bit_width(value);
    if (width == 0) {
        return 0;
    }
    return 1ULL << (width - 1);
}

int64_t int_highest_one(int64_t value) {
    return (int64_t)usize_highest_one((uint64_t)value);
}

uint64_t usize_clear_lowest_one(uint64_t value) {
    if (value == 0) {
        return 0;
    }
    return value & (value - 1ULL);
}

int64_t int_clear_lowest_one(int64_t value) {
    return (int64_t)usize_clear_lowest_one((uint64_t)value);
}

uint64_t usize_clear_highest_one(uint64_t value) {
    return value - usize_highest_one(value);
}

int64_t int_clear_highest_one(int64_t value) {
    return (int64_t)usize_clear_highest_one((uint64_t)value);
}

uint64_t usize_fill_ones_below(uint64_t value) {
    if (value == 0) {
        return 0;
    }
    return usize_low_mask(usize_bit_width(value));
}

int64_t int_fill_ones_below(int64_t value) {
    return (int64_t)usize_fill_ones_below((uint64_t)value);
}

uint64_t usize_fill_ones_above(uint64_t value) {
    if (value == 0) {
        return 0;
    }
    return ~usize_low_mask(usize_trailing_zeros(value));
}

int64_t int_fill_ones_above(int64_t value) {
    return (int64_t)usize_fill_ones_above((uint64_t)value);
}

uint64_t usize_rotate_left(uint64_t value, uint64_t shift) {
    shift &= 63;
    if (shift == 0) {
        return value;
    }
    return (value << shift) | (value >> (64 - shift));
}

int64_t int_rotate_left(int64_t value, uint64_t shift) {
    return (int64_t)usize_rotate_left((uint64_t)value, shift);
}

uint64_t usize_rotate_right(uint64_t value, uint64_t shift) {
    shift &= 63;
    if (shift == 0) {
        return value;
    }
    return (value >> shift) | (value << (64 - shift));
}

int64_t int_rotate_right(int64_t value, uint64_t shift) {
    return (int64_t)usize_rotate_right((uint64_t)value, shift);
}

uint64_t usize_checked_shl(uint64_t value, uint64_t shift) {
    if (shift >= 64) {
        return 0;
    }
    return value << shift;
}

uint64_t usize_checked_shr(uint64_t value, uint64_t shift) {
    if (shift >= 64) {
        return 0;
    }
    return value >> shift;
}

uint64_t usize_wrapping_shl(uint64_t value, uint64_t shift) {
    return value << (shift & 63);
}

uint64_t usize_wrapping_shr(uint64_t value, uint64_t shift) {
    return value >> (shift & 63);
}

int64_t int_checked_shl(int64_t value, uint64_t shift) {
    if (shift >= 64) {
        return 0;
    }
    return (int64_t)((uint64_t)value << shift);
}

int64_t int_checked_shr(int64_t value, uint64_t shift) {
    if (shift >= 64) {
        return 0;
    }
    return (int64_t)((uint64_t)value >> shift);
}

int64_t int_wrapping_shl(int64_t value, uint64_t shift) {
    return (int64_t)((uint64_t)value << (shift & 63));
}

int64_t int_wrapping_shr(int64_t value, uint64_t shift) {
    return (int64_t)((uint64_t)value >> (shift & 63));
}

int64_t int_arithmetic_shr(int64_t value, uint64_t shift) {
    return value >> (shift & 63);
}

int usize_bit_is_set(uint64_t value, uint64_t bit) {
    if (bit >= 64) {
        return 0;
    }
    return ((value >> bit) & 1u) != 0;
}

int int_bit_is_set(int64_t value, uint64_t bit) {
    return usize_bit_is_set((uint64_t)value, bit);
}

int usize_bits_contains_all(uint64_t value, uint64_t mask) {
    return (value & mask) == mask;
}

int int_bits_contains_all(int64_t value, int64_t mask) {
    return usize_bits_contains_all((uint64_t)value, (uint64_t)mask);
}

int usize_bits_disjoint(uint64_t value, uint64_t mask) {
    return (value & mask) == 0;
}

int int_bits_disjoint(int64_t value, int64_t mask) {
    return usize_bits_disjoint((uint64_t)value, (uint64_t)mask);
}

uint64_t usize_bit_set(uint64_t value, uint64_t bit) {
    if (bit >= 64) {
        return value;
    }
    return value | (1ULL << bit);
}

uint64_t usize_low_mask(uint64_t bits) {
    if (bits == 0) {
        return 0;
    }
    if (bits >= 64) {
        return UINT64_MAX;
    }
    return (1ULL << bits) - 1ULL;
}

uint64_t usize_range_mask(uint64_t start, uint64_t len) {
    if (start >= 64 || len == 0) {
        return 0;
    }
    uint64_t available = 64 - start;
    uint64_t width = len < available ? len : available;
    return usize_low_mask(width) << start;
}

uint64_t usize_extract_bits(uint64_t value, uint64_t start, uint64_t len) {
    if (start >= 64 || len == 0) {
        return 0;
    }
    uint64_t available = 64 - start;
    uint64_t width = len < available ? len : available;
    return (value >> start) & usize_low_mask(width);
}

uint64_t usize_insert_bits(uint64_t value, uint64_t insert, uint64_t start, uint64_t len) {
    if (start >= 64 || len == 0) {
        return value;
    }
    uint64_t available = 64 - start;
    uint64_t width = len < available ? len : available;
    uint64_t low_mask = usize_low_mask(width);
    uint64_t field_mask = low_mask << start;
    return (value & ~field_mask) | ((insert & low_mask) << start);
}

uint8_t usize_byte_at(uint64_t value, uint64_t index) {
    if (index >= 8) {
        return 0;
    }
    return (uint8_t)((value >> (index * 8)) & 0xffu);
}

uint64_t usize_with_byte(uint64_t value, uint64_t index, uint8_t byte) {
    if (index >= 8) {
        return value;
    }
    uint64_t shift = index * 8;
    uint64_t mask = 0xffULL << shift;
    return (value & ~mask) | ((uint64_t)byte << shift);
}

int64_t int_bit_set(int64_t value, uint64_t bit) {
    return (int64_t)usize_bit_set((uint64_t)value, bit);
}

int64_t int_low_mask(uint64_t bits) {
    return (int64_t)usize_low_mask(bits);
}

int64_t int_range_mask(uint64_t start, uint64_t len) {
    return (int64_t)usize_range_mask(start, len);
}

int64_t int_sign_extend(int64_t value, uint64_t bits) {
    if (bits == 0) {
        return 0;
    }
    if (bits >= 64) {
        return value;
    }
    uint64_t sign_bit = 1ULL << (bits - 1);
    uint64_t mask = usize_low_mask(bits);
    uint64_t truncated = (uint64_t)value & mask;
    if ((truncated & sign_bit) == 0) {
        return (int64_t)truncated;
    }
    return (int64_t)(truncated | ~mask);
}

int64_t int_extract_bits(int64_t value, uint64_t start, uint64_t len) {
    return (int64_t)usize_extract_bits((uint64_t)value, start, len);
}

int64_t int_insert_bits(int64_t value, int64_t insert, uint64_t start, uint64_t len) {
    return (int64_t)usize_insert_bits((uint64_t)value, (uint64_t)insert, start, len);
}

uint8_t int_byte_at(int64_t value, uint64_t index) {
    return usize_byte_at((uint64_t)value, index);
}

int64_t int_with_byte(int64_t value, uint64_t index, uint8_t byte) {
    return (int64_t)usize_with_byte((uint64_t)value, index, byte);
}

uint64_t usize_bit_clear(uint64_t value, uint64_t bit) {
    if (bit >= 64) {
        return value;
    }
    return value & ~(1ULL << bit);
}

int64_t int_bit_clear(int64_t value, uint64_t bit) {
    return (int64_t)usize_bit_clear((uint64_t)value, bit);
}

uint64_t usize_bit_toggle(uint64_t value, uint64_t bit) {
    if (bit >= 64) {
        return value;
    }
    return value ^ (1ULL << bit);
}

int64_t int_bit_toggle(int64_t value, uint64_t bit) {
    return (int64_t)usize_bit_toggle((uint64_t)value, bit);
}

int __geo_bounds_check(uint64_t index, uint64_t len) {
    if (index >= len) {
        fputs("Geo bounds check failed\n", stderr);
        exit(101);
    }
    return 0;
}

int exit_geo(int code) {
    exit(code);
}

int arg_count(void) {
    return geo_argc;
}

const char *arg(int index) {
    if (index < 0 || index >= geo_argc || geo_argv == NULL || geo_argv[index] == NULL) {
        return "";
    }
    return geo_argv[index];
}

int arg_exists(int index) {
    return index >= 0 && index < geo_argc && geo_argv != NULL && geo_argv[index] != NULL;
}

const char *arg_or(int index, const char *default_value) {
    if (!arg_exists(index)) {
        return default_value == NULL ? "" : default_value;
    }
    return geo_argv[index];
}

const char *env_get(const char *name) {
    if (name == NULL) {
        return "";
    }
    const char *value = getenv(name);
    return value == NULL ? "" : value;
}

const char *env_get_or(const char *name, const char *default_value) {
    if (name == NULL || name[0] == '\0') {
        return default_value == NULL ? "" : default_value;
    }
    const char *value = getenv(name);
    return value == NULL ? (default_value == NULL ? "" : default_value) : value;
}

int env_exists(const char *name) {
    if (name == NULL || name[0] == '\0') {
        return 0;
    }
    return getenv(name) != NULL;
}

uint64_t env_count(void) {
    uint64_t count = 0;
    if (geo_environ == NULL) {
        return 0;
    }
    while (geo_environ[count] != NULL) {
        count++;
    }
    return count;
}

const char *env_name(uint64_t index) {
    if (geo_environ == NULL) {
        return "";
    }
    const char *entry = geo_environ[index];
    if (entry == NULL) {
        return "";
    }
    const char *equals = strchr(entry, '=');
    if (equals == NULL) {
        return geo_string_slice(entry, strlen(entry));
    }
    return geo_string_slice(entry, (size_t)(equals - entry));
}

const char *env_value(uint64_t index) {
    if (geo_environ == NULL) {
        return "";
    }
    const char *entry = geo_environ[index];
    if (entry == NULL) {
        return "";
    }
    const char *equals = strchr(entry, '=');
    if (equals == NULL) {
        return "";
    }
    equals++;
    return geo_string_slice(equals, strlen(equals));
}

int env_set(const char *name, const char *value) {
    if (name == NULL || name[0] == '\0') {
        return 1;
    }
    if (value == NULL) {
        value = "";
    }
    return geo_setenv(name, value);
}

int env_remove(const char *name) {
    if (name == NULL || name[0] == '\0') {
        return 1;
    }
    return geo_unsetenv(name);
}

int run_command(const char *command) {
    if (command == NULL || command[0] == '\0') {
        return -1;
    }
    int status = system(command);
    if (status == -1) {
        return -1;
    }
#if defined(_WIN32)
    return status;
#else
    if (WIFEXITED(status)) {
        return WEXITSTATUS(status);
    }
    if (WIFSIGNALED(status)) {
        return 128 + WTERMSIG(status);
    }
    return status;
#endif
}

uint64_t process_id(void) {
#if defined(_WIN32)
    return (uint64_t)GetCurrentProcessId();
#else
    return (uint64_t)getpid();
#endif
}

const char *current_exe(void) {
#if defined(_WIN32)
    DWORD capacity = 260;
    for (;;) {
        char *buffer = (char *)malloc((size_t)capacity);
        if (buffer == NULL) {
            return "";
        }
        DWORD len = GetModuleFileNameA(NULL, buffer, capacity);
        if (len == 0) {
            free(buffer);
            return "";
        }
        if (len < capacity - 1) {
            return buffer;
        }
        free(buffer);
        capacity *= 2;
    }
#else
    size_t capacity = 256;
    for (;;) {
        char *buffer = (char *)malloc(capacity);
        if (buffer == NULL) {
            return "";
        }
        ssize_t len = readlink("/proc/self/exe", buffer, capacity - 1);
        if (len < 0) {
            free(buffer);
            return "";
        }
        if ((size_t)len < capacity - 1) {
            buffer[len] = '\0';
            return buffer;
        }
        free(buffer);
        capacity *= 2;
    }
#endif
}

uint64_t string_len(const char *value) {
    return value == NULL ? 0 : (uint64_t)strlen(value);
}

int string_byte_at(const char *value, uint64_t index) {
    if (value == NULL) {
        return -1;
    }
    size_t len = strlen(value);
    if (index >= (uint64_t)len) {
        return -1;
    }
    return (int)((const unsigned char *)value)[index];
}

const char *string_from_byte(int value) {
    if (value <= 0 || value > 255) {
        return "";
    }
    char *out = (char *)malloc(2);
    if (out == NULL) {
        return "";
    }
    out[0] = (char)(unsigned char)value;
    out[1] = '\0';
    return out;
}

const char *string_from_utf8_codepoint(int value) {
    if (value <= 0 || value > 0x10ffff || (value >= 0xd800 && value <= 0xdfff)) {
        return "";
    }
    uint32_t scalar = (uint32_t)value;
    char buffer[5];
    size_t len = 0;
    if (scalar <= 0x7f) {
        buffer[0] = (char)scalar;
        len = 1;
    } else if (scalar <= 0x7ff) {
        buffer[0] = (char)(0xc0 | (scalar >> 6));
        buffer[1] = (char)(0x80 | (scalar & 0x3f));
        len = 2;
    } else if (scalar <= 0xffff) {
        buffer[0] = (char)(0xe0 | (scalar >> 12));
        buffer[1] = (char)(0x80 | ((scalar >> 6) & 0x3f));
        buffer[2] = (char)(0x80 | (scalar & 0x3f));
        len = 3;
    } else {
        buffer[0] = (char)(0xf0 | (scalar >> 18));
        buffer[1] = (char)(0x80 | ((scalar >> 12) & 0x3f));
        buffer[2] = (char)(0x80 | ((scalar >> 6) & 0x3f));
        buffer[3] = (char)(0x80 | (scalar & 0x3f));
        len = 4;
    }
    buffer[len] = '\0';
    return geo_string_slice(buffer, len);
}

int string_find_byte(const char *value, int byte) {
    if (value == NULL || byte < 0 || byte > 255) {
        return -1;
    }
    const unsigned char *cursor = (const unsigned char *)value;
    for (uint64_t index = 0; *cursor != '\0'; index++) {
        if (*cursor == (unsigned char)byte) {
            if (index > (uint64_t)INT32_MAX) {
                return -1;
            }
            return (int)index;
        }
        cursor++;
    }
    return -1;
}

int string_last_find_byte(const char *value, int byte) {
    if (value == NULL || byte < 0 || byte > 255) {
        return -1;
    }
    int last = -1;
    const unsigned char *cursor = (const unsigned char *)value;
    for (uint64_t index = 0; *cursor != '\0'; index++) {
        if (*cursor == (unsigned char)byte) {
            if (index > (uint64_t)INT32_MAX) {
                return -1;
            }
            last = (int)index;
        }
        cursor++;
    }
    return last;
}

int string_is_empty(const char *value) {
    return value == NULL || value[0] == '\0';
}

int string_is_ascii(const char *value) {
    if (value == NULL) {
        return 1;
    }
    const unsigned char *cursor = (const unsigned char *)value;
    while (*cursor != '\0') {
        if (*cursor > 0x7f) {
            return 0;
        }
        cursor++;
    }
    return 1;
}

int string_is_utf8(const char *value) {
    if (value == NULL) {
        return 1;
    }
    const unsigned char *cursor = (const unsigned char *)value;
    while (*cursor != '\0') {
        uint32_t scalar = 0;
        size_t needed = 0;
        unsigned char first = *cursor;
        if (first <= 0x7f) {
            cursor++;
            continue;
        }
        if (first >= 0xc2 && first <= 0xdf) {
            scalar = (uint32_t)(first & 0x1f);
            needed = 1;
        } else if (first >= 0xe0 && first <= 0xef) {
            scalar = (uint32_t)(first & 0x0f);
            needed = 2;
        } else if (first >= 0xf0 && first <= 0xf4) {
            scalar = (uint32_t)(first & 0x07);
            needed = 3;
        } else {
            return 0;
        }
        cursor++;
        for (size_t i = 0; i < needed; i++) {
            if ((cursor[i] & 0xc0) != 0x80) {
                return 0;
            }
            scalar = (scalar << 6) | (uint32_t)(cursor[i] & 0x3f);
        }
        if ((needed == 2 && scalar < 0x800) || (needed == 3 && scalar < 0x10000) ||
            scalar > 0x10ffff || (scalar >= 0xd800 && scalar <= 0xdfff)) {
            return 0;
        }
        cursor += needed;
    }
    return 1;
}

int string_utf8_is_valid(const char *value) {
    return string_is_utf8(value);
}

int string_utf8_len(const char *value) {
    if (value == NULL) {
        return 0;
    }
    const unsigned char *cursor = (const unsigned char *)value;
    uint64_t count = 0;
    while (*cursor != '\0') {
        uint32_t scalar = 0;
        size_t needed = 0;
        unsigned char first = *cursor;
        if (first <= 0x7f) {
            cursor++;
            count++;
            continue;
        }
        if (first >= 0xc2 && first <= 0xdf) {
            scalar = (uint32_t)(first & 0x1f);
            needed = 1;
        } else if (first >= 0xe0 && first <= 0xef) {
            scalar = (uint32_t)(first & 0x0f);
            needed = 2;
        } else if (first >= 0xf0 && first <= 0xf4) {
            scalar = (uint32_t)(first & 0x07);
            needed = 3;
        } else {
            return -1;
        }
        cursor++;
        for (size_t i = 0; i < needed; i++) {
            if ((cursor[i] & 0xc0) != 0x80) {
                return -1;
            }
            scalar = (scalar << 6) | (uint32_t)(cursor[i] & 0x3f);
        }
        if ((needed == 2 && scalar < 0x800) || (needed == 3 && scalar < 0x10000) ||
            scalar > 0x10ffff || (scalar >= 0xd800 && scalar <= 0xdfff)) {
            return -1;
        }
        if (count >= (uint64_t)INT32_MAX) {
            return -1;
        }
        cursor += needed;
        count++;
    }
    return (int)count;
}

int string_utf8_byte_offset(const char *value, uint64_t index) {
    if (value == NULL) {
        return index == 0 ? 0 : -1;
    }
    const unsigned char *start = (const unsigned char *)value;
    const unsigned char *cursor = start;
    uint64_t count = 0;
    while (*cursor != '\0') {
        if (count == index) {
            uint64_t offset = (uint64_t)(cursor - start);
            return offset <= (uint64_t)INT32_MAX ? (int)offset : -1;
        }

        uint32_t scalar = 0;
        size_t needed = 0;
        unsigned char first = *cursor;
        if (first <= 0x7f) {
            cursor++;
            count++;
            continue;
        }
        if (first >= 0xc2 && first <= 0xdf) {
            scalar = (uint32_t)(first & 0x1f);
            needed = 1;
        } else if (first >= 0xe0 && first <= 0xef) {
            scalar = (uint32_t)(first & 0x0f);
            needed = 2;
        } else if (first >= 0xf0 && first <= 0xf4) {
            scalar = (uint32_t)(first & 0x07);
            needed = 3;
        } else {
            return -1;
        }
        cursor++;
        for (size_t i = 0; i < needed; i++) {
            if ((cursor[i] & 0xc0) != 0x80) {
                return -1;
            }
            scalar = (scalar << 6) | (uint32_t)(cursor[i] & 0x3f);
        }
        if ((needed == 2 && scalar < 0x800) || (needed == 3 && scalar < 0x10000) ||
            scalar > 0x10ffff || (scalar >= 0xd800 && scalar <= 0xdfff)) {
            return -1;
        }
        cursor += needed;
        count++;
    }
    if (count == index) {
        uint64_t offset = (uint64_t)(cursor - start);
        return offset <= (uint64_t)INT32_MAX ? (int)offset : -1;
    }
    return -1;
}

const char *string_utf8_char_at(const char *value, uint64_t index) {
    if (value == NULL || index == UINT64_MAX) {
        return "";
    }
    int start_offset = string_utf8_byte_offset(value, index);
    int end_offset = string_utf8_byte_offset(value, index + 1);
    if (start_offset < 0 || end_offset < start_offset) {
        return "";
    }
    return geo_string_slice(value + start_offset, (size_t)(end_offset - start_offset));
}

int string_utf8_codepoint_at(const char *value, uint64_t index) {
    if (value == NULL) {
        return -1;
    }
    int offset = string_utf8_byte_offset(value, index);
    if (offset < 0) {
        return -1;
    }
    const unsigned char *cursor = (const unsigned char *)value + offset;
    unsigned char first = *cursor;
    if (first == '\0') {
        return -1;
    }
    if (first <= 0x7f) {
        return (int)first;
    }

    uint32_t scalar = 0;
    size_t needed = 0;
    if (first >= 0xc2 && first <= 0xdf) {
        scalar = (uint32_t)(first & 0x1f);
        needed = 1;
    } else if (first >= 0xe0 && first <= 0xef) {
        scalar = (uint32_t)(first & 0x0f);
        needed = 2;
    } else if (first >= 0xf0 && first <= 0xf4) {
        scalar = (uint32_t)(first & 0x07);
        needed = 3;
    } else {
        return -1;
    }

    cursor++;
    for (size_t i = 0; i < needed; i++) {
        if ((cursor[i] & 0xc0) != 0x80) {
            return -1;
        }
        scalar = (scalar << 6) | (uint32_t)(cursor[i] & 0x3f);
    }
    if ((needed == 2 && scalar < 0x800) || (needed == 3 && scalar < 0x10000) ||
        scalar > 0x10ffff || (scalar >= 0xd800 && scalar <= 0xdfff)) {
        return -1;
    }
    return scalar <= (uint32_t)INT32_MAX ? (int)scalar : -1;
}

int string_utf8_index_at(const char *value, uint64_t offset) {
    if (value == NULL) {
        return offset == 0 ? 0 : -1;
    }
    const unsigned char *start = (const unsigned char *)value;
    const unsigned char *cursor = start;
    uint64_t count = 0;
    while (*cursor != '\0') {
        uint64_t current_offset = (uint64_t)(cursor - start);
        if (current_offset == offset) {
            return count <= (uint64_t)INT32_MAX ? (int)count : -1;
        }
        if (current_offset > offset) {
            return -1;
        }

        uint32_t scalar = 0;
        size_t needed = 0;
        unsigned char first = *cursor;
        if (first <= 0x7f) {
            cursor++;
            count++;
            continue;
        }
        if (first >= 0xc2 && first <= 0xdf) {
            scalar = (uint32_t)(first & 0x1f);
            needed = 1;
        } else if (first >= 0xe0 && first <= 0xef) {
            scalar = (uint32_t)(first & 0x0f);
            needed = 2;
        } else if (first >= 0xf0 && first <= 0xf4) {
            scalar = (uint32_t)(first & 0x07);
            needed = 3;
        } else {
            return -1;
        }
        cursor++;
        for (size_t i = 0; i < needed; i++) {
            if ((cursor[i] & 0xc0) != 0x80) {
                return -1;
            }
            scalar = (scalar << 6) | (uint32_t)(cursor[i] & 0x3f);
        }
        if ((needed == 2 && scalar < 0x800) || (needed == 3 && scalar < 0x10000) ||
            scalar > 0x10ffff || (scalar >= 0xd800 && scalar <= 0xdfff)) {
            return -1;
        }
        cursor += needed;
        count++;
    }
    if ((uint64_t)(cursor - start) == offset) {
        return count <= (uint64_t)INT32_MAX ? (int)count : -1;
    }
    return -1;
}

int string_utf8_next_offset(const char *value, uint64_t offset) {
    if (value == NULL) {
        return offset == 0 ? 0 : -1;
    }
    int index = string_utf8_index_at(value, offset);
    if (index < 0) {
        return -1;
    }
    uint64_t len = (uint64_t)strlen(value);
    if (offset == len) {
        return len <= (uint64_t)INT32_MAX ? (int)len : -1;
    }
    return string_utf8_byte_offset(value, (uint64_t)index + 1);
}

int string_utf8_prev_offset(const char *value, uint64_t offset) {
    if (value == NULL) {
        return offset == 0 ? 0 : -1;
    }
    int index = string_utf8_index_at(value, offset);
    if (index < 0) {
        return -1;
    }
    if (index == 0) {
        return 0;
    }
    return string_utf8_byte_offset(value, (uint64_t)index - 1);
}

int string_utf8_is_boundary(const char *value, uint64_t offset) {
    return string_utf8_index_at(value, offset) >= 0;
}

int string_utf8_find_codepoint(const char *value, int codepoint) {
    if (value == NULL || codepoint <= 0 || codepoint > 0x10ffff ||
        (codepoint >= 0xd800 && codepoint <= 0xdfff)) {
        return -1;
    }
    int scalar_len = string_utf8_len(value);
    if (scalar_len < 0) {
        return -1;
    }
    for (int index = 0; index < scalar_len; index++) {
        int scalar = string_utf8_codepoint_at(value, (uint64_t)index);
        if (scalar == codepoint) {
            return string_utf8_byte_offset(value, (uint64_t)index);
        }
    }
    return -1;
}

int string_utf8_contains_codepoint(const char *value, int codepoint) {
    return string_utf8_find_codepoint(value, codepoint) >= 0;
}

int string_utf8_starts_with_codepoint(const char *value, int codepoint) {
    if (value == NULL || codepoint <= 0 || codepoint > 0x10ffff ||
        (codepoint >= 0xd800 && codepoint <= 0xdfff)) {
        return 0;
    }
    return string_utf8_codepoint_at(value, 0) == codepoint ? 1 : 0;
}

int string_utf8_ends_with_codepoint(const char *value, int codepoint) {
    if (value == NULL || codepoint <= 0 || codepoint > 0x10ffff ||
        (codepoint >= 0xd800 && codepoint <= 0xdfff)) {
        return 0;
    }
    int scalar_len = string_utf8_len(value);
    if (scalar_len <= 0) {
        return 0;
    }
    return string_utf8_codepoint_at(value, (uint64_t)scalar_len - 1) == codepoint ? 1 : 0;
}

uint64_t string_utf8_count_codepoint(const char *value, int codepoint) {
    if (value == NULL || codepoint <= 0 || codepoint > 0x10ffff ||
        (codepoint >= 0xd800 && codepoint <= 0xdfff)) {
        return 0;
    }
    int scalar_len = string_utf8_len(value);
    if (scalar_len < 0) {
        return 0;
    }
    uint64_t count = 0;
    for (int index = 0; index < scalar_len; index++) {
        if (string_utf8_codepoint_at(value, (uint64_t)index) == codepoint) {
            count++;
        }
    }
    return count;
}

int string_utf8_last_find_codepoint(const char *value, int codepoint) {
    if (value == NULL || codepoint <= 0 || codepoint > 0x10ffff ||
        (codepoint >= 0xd800 && codepoint <= 0xdfff)) {
        return -1;
    }
    int scalar_len = string_utf8_len(value);
    if (scalar_len < 0) {
        return -1;
    }
    int last = -1;
    for (int index = 0; index < scalar_len; index++) {
        int scalar = string_utf8_codepoint_at(value, (uint64_t)index);
        if (scalar == codepoint) {
            int offset = string_utf8_byte_offset(value, (uint64_t)index);
            if (offset < 0) {
                return -1;
            }
            last = offset;
        }
    }
    return last;
}

int string_is_ascii_digit(const char *value) {
    if (value == NULL || value[0] == '\0') {
        return 0;
    }
    const unsigned char *cursor = (const unsigned char *)value;
    while (*cursor != '\0') {
        if (*cursor < '0' || *cursor > '9') {
            return 0;
        }
        cursor++;
    }
    return 1;
}

int string_is_ascii_hex_digit(const char *value) {
    if (value == NULL || value[0] == '\0') {
        return 0;
    }
    const unsigned char *cursor = (const unsigned char *)value;
    while (*cursor != '\0') {
        int is_digit = *cursor >= '0' && *cursor <= '9';
        int is_upper_hex = *cursor >= 'A' && *cursor <= 'F';
        int is_lower_hex = *cursor >= 'a' && *cursor <= 'f';
        if (!is_digit && !is_upper_hex && !is_lower_hex) {
            return 0;
        }
        cursor++;
    }
    return 1;
}

int string_is_ascii_alpha(const char *value) {
    if (value == NULL || value[0] == '\0') {
        return 0;
    }
    const unsigned char *cursor = (const unsigned char *)value;
    while (*cursor != '\0') {
        if (!((*cursor >= 'A' && *cursor <= 'Z') || (*cursor >= 'a' && *cursor <= 'z'))) {
            return 0;
        }
        cursor++;
    }
    return 1;
}

int string_is_ascii_lower(const char *value) {
    if (value == NULL || value[0] == '\0') {
        return 0;
    }
    const unsigned char *cursor = (const unsigned char *)value;
    while (*cursor != '\0') {
        if (*cursor < 'a' || *cursor > 'z') {
            return 0;
        }
        cursor++;
    }
    return 1;
}

int string_is_ascii_upper(const char *value) {
    if (value == NULL || value[0] == '\0') {
        return 0;
    }
    const unsigned char *cursor = (const unsigned char *)value;
    while (*cursor != '\0') {
        if (*cursor < 'A' || *cursor > 'Z') {
            return 0;
        }
        cursor++;
    }
    return 1;
}

int string_is_ascii_alnum(const char *value) {
    if (value == NULL || value[0] == '\0') {
        return 0;
    }
    const unsigned char *cursor = (const unsigned char *)value;
    while (*cursor != '\0') {
        int is_alpha = (*cursor >= 'A' && *cursor <= 'Z') || (*cursor >= 'a' && *cursor <= 'z');
        int is_digit = *cursor >= '0' && *cursor <= '9';
        if (!is_alpha && !is_digit) {
            return 0;
        }
        cursor++;
    }
    return 1;
}

int string_is_ascii_identifier(const char *value) {
    if (value == NULL || value[0] == '\0') {
        return 0;
    }
    const unsigned char *cursor = (const unsigned char *)value;
    if (!((*cursor >= 'A' && *cursor <= 'Z') || (*cursor >= 'a' && *cursor <= 'z') ||
          *cursor == '_')) {
        return 0;
    }
    cursor++;
    while (*cursor != '\0') {
        if (!((*cursor >= 'A' && *cursor <= 'Z') || (*cursor >= 'a' && *cursor <= 'z') ||
              (*cursor >= '0' && *cursor <= '9') || *cursor == '_')) {
            return 0;
        }
        cursor++;
    }
    return 1;
}

int string_is_ascii_whitespace(const char *value) {
    if (value == NULL || value[0] == '\0') {
        return 0;
    }
    const char *cursor = value;
    while (*cursor != '\0') {
        if (!geo_is_space(*cursor)) {
            return 0;
        }
        cursor++;
    }
    return 1;
}

int __geo_string_get(const char *value, uint64_t index) {
    uint64_t len = value == NULL ? 0 : (uint64_t)strlen(value);
    __geo_bounds_check(index, len);
    return (unsigned char)value[index];
}

const char *string_clone(const char *value) {
    if (value == NULL) {
        return NULL;
    }
    size_t len = strlen(value);
    char *copy_value = (char *)malloc(len + 1);
    if (copy_value == NULL) {
        return NULL;
    }
    memcpy(copy_value, value, len + 1);
    return copy_value;
}

int string_compare(const char *left, const char *right) {
    if (left == NULL || right == NULL) {
        return left == right ? 0 : (left == NULL ? -1 : 1);
    }
    return strcmp(left, right);
}

int string_contains(const char *value, const char *needle) {
    if (value == NULL || needle == NULL) {
        return 0;
    }
    return strstr(value, needle) != NULL ? 1 : 0;
}

int string_eq(const char *left, const char *right) {
    return string_compare(left, right) == 0 ? 1 : 0;
}

int string_not_eq(const char *left, const char *right) {
    return string_compare(left, right) != 0 ? 1 : 0;
}

int string_less(const char *left, const char *right) {
    return string_compare(left, right) < 0 ? 1 : 0;
}

int string_less_or_equal(const char *left, const char *right) {
    return string_compare(left, right) <= 0 ? 1 : 0;
}

int string_greater(const char *left, const char *right) {
    return string_compare(left, right) > 0 ? 1 : 0;
}

int string_greater_or_equal(const char *left, const char *right) {
    return string_compare(left, right) >= 0 ? 1 : 0;
}

static unsigned char geo_ascii_lower_byte(unsigned char byte) {
    return (byte >= 'A' && byte <= 'Z') ? (unsigned char)(byte + ('a' - 'A')) : byte;
}

int string_compare_ignore_case(const char *left, const char *right) {
    if (left == NULL || right == NULL) {
        return left == right ? 0 : (left == NULL ? -1 : 1);
    }
    while (*left != '\0' && *right != '\0') {
        unsigned char left_byte = geo_ascii_lower_byte((unsigned char)*left);
        unsigned char right_byte = geo_ascii_lower_byte((unsigned char)*right);
        if (left_byte != right_byte) {
            return left_byte < right_byte ? -1 : 1;
        }
        left++;
        right++;
    }
    unsigned char left_byte = geo_ascii_lower_byte((unsigned char)*left);
    unsigned char right_byte = geo_ascii_lower_byte((unsigned char)*right);
    if (left_byte == right_byte) {
        return 0;
    }
    return left_byte < right_byte ? -1 : 1;
}

int string_eq_ignore_case(const char *left, const char *right) {
    return string_compare_ignore_case(left, right) == 0 ? 1 : 0;
}

int string_not_eq_ignore_case(const char *left, const char *right) {
    return string_compare_ignore_case(left, right) != 0 ? 1 : 0;
}

int string_less_ignore_case(const char *left, const char *right) {
    return string_compare_ignore_case(left, right) < 0 ? 1 : 0;
}

int string_less_or_equal_ignore_case(const char *left, const char *right) {
    return string_compare_ignore_case(left, right) <= 0 ? 1 : 0;
}

int string_greater_ignore_case(const char *left, const char *right) {
    return string_compare_ignore_case(left, right) > 0 ? 1 : 0;
}

int string_greater_or_equal_ignore_case(const char *left, const char *right) {
    return string_compare_ignore_case(left, right) >= 0 ? 1 : 0;
}

int string_index_of(const char *value, const char *needle) {
    if (value == NULL || needle == NULL) {
        return -1;
    }
    const char *match = strstr(value, needle);
    if (match == NULL) {
        return -1;
    }
    return (int)(match - value);
}

int string_last_index_of(const char *value, const char *needle) {
    if (value == NULL || needle == NULL) {
        return -1;
    }
    size_t needle_len = strlen(needle);
    if (needle_len == 0) {
        return -1;
    }
    const char *last = NULL;
    const char *cursor = value;
    while ((cursor = strstr(cursor, needle)) != NULL) {
        last = cursor;
        cursor += 1;
    }
    return last == NULL ? -1 : (int)(last - value);
}

uint64_t string_count(const char *value, const char *needle) {
    if (value == NULL || needle == NULL) {
        return 0;
    }
    size_t needle_len = strlen(needle);
    if (needle_len == 0) {
        return 0;
    }
    uint64_t count = 0;
    const char *cursor = value;
    while ((cursor = strstr(cursor, needle)) != NULL) {
        count++;
        cursor += needle_len;
    }
    return count;
}

const char *string_before(const char *value, const char *delimiter) {
    if (value == NULL) {
        return "";
    }
    if (delimiter == NULL) {
        return string_clone(value);
    }
    size_t delimiter_len = strlen(delimiter);
    if (delimiter_len == 0) {
        return "";
    }
    const char *match = strstr(value, delimiter);
    if (match == NULL) {
        return string_clone(value);
    }
    return geo_string_slice(value, (size_t)(match - value));
}

const char *string_after(const char *value, const char *delimiter) {
    if (value == NULL) {
        return "";
    }
    if (delimiter == NULL) {
        return "";
    }
    size_t delimiter_len = strlen(delimiter);
    if (delimiter_len == 0) {
        return string_clone(value);
    }
    const char *match = strstr(value, delimiter);
    if (match == NULL) {
        return "";
    }
    return string_clone(match + delimiter_len);
}

const char *string_before_last(const char *value, const char *delimiter) {
    if (value == NULL) {
        return "";
    }
    if (delimiter == NULL) {
        return string_clone(value);
    }
    size_t delimiter_len = strlen(delimiter);
    if (delimiter_len == 0) {
        return "";
    }
    const char *last = NULL;
    const char *cursor = value;
    while ((cursor = strstr(cursor, delimiter)) != NULL) {
        last = cursor;
        cursor += 1;
    }
    if (last == NULL) {
        return string_clone(value);
    }
    return geo_string_slice(value, (size_t)(last - value));
}

const char *string_after_last(const char *value, const char *delimiter) {
    if (value == NULL) {
        return "";
    }
    if (delimiter == NULL) {
        return "";
    }
    size_t delimiter_len = strlen(delimiter);
    if (delimiter_len == 0) {
        return string_clone(value);
    }
    const char *last = NULL;
    const char *cursor = value;
    while ((cursor = strstr(cursor, delimiter)) != NULL) {
        last = cursor;
        cursor += 1;
    }
    if (last == NULL) {
        return "";
    }
    return string_clone(last + delimiter_len);
}

const char *string_strip_prefix(const char *value, const char *prefix) {
    if (value == NULL) {
        return "";
    }
    if (prefix == NULL) {
        return string_clone(value);
    }
    size_t prefix_len = strlen(prefix);
    if (prefix_len == 0) {
        return string_clone(value);
    }
    if (strncmp(value, prefix, prefix_len) != 0) {
        return string_clone(value);
    }
    return string_clone(value + prefix_len);
}

const char *string_strip_suffix(const char *value, const char *suffix) {
    if (value == NULL) {
        return "";
    }
    if (suffix == NULL) {
        return string_clone(value);
    }
    size_t value_len = strlen(value);
    size_t suffix_len = strlen(suffix);
    if (suffix_len == 0 || suffix_len > value_len) {
        return string_clone(value);
    }
    if (memcmp(value + value_len - suffix_len, suffix, suffix_len) != 0) {
        return string_clone(value);
    }
    return geo_string_slice(value, value_len - suffix_len);
}

const char *string_between(const char *value, const char *start, const char *end) {
    if (value == NULL || start == NULL || end == NULL) {
        return "";
    }
    size_t start_len = strlen(start);
    size_t end_len = strlen(end);
    if (start_len == 0 || end_len == 0) {
        return "";
    }
    const char *start_match = strstr(value, start);
    if (start_match == NULL) {
        return "";
    }
    const char *content = start_match + start_len;
    const char *end_match = strstr(content, end);
    if (end_match == NULL) {
        return "";
    }
    return geo_string_slice(content, (size_t)(end_match - content));
}

const char *string_between_last(const char *value, const char *start, const char *end) {
    if (value == NULL || start == NULL || end == NULL) {
        return "";
    }
    size_t start_len = strlen(start);
    size_t end_len = strlen(end);
    if (start_len == 0 || end_len == 0) {
        return "";
    }
    const char *best_content = NULL;
    const char *best_end = NULL;
    const char *cursor = value;
    while ((cursor = strstr(cursor, start)) != NULL) {
        const char *content = cursor + start_len;
        const char *end_match = strstr(content, end);
        if (end_match != NULL) {
            best_content = content;
            best_end = end_match;
        }
        cursor += 1;
    }
    if (best_content == NULL || best_end == NULL) {
        return "";
    }
    return geo_string_slice(best_content, (size_t)(best_end - best_content));
}

int string_starts_with(const char *value, const char *prefix) {
    if (value == NULL || prefix == NULL) {
        return 0;
    }
    size_t prefix_len = strlen(prefix);
    return strncmp(value, prefix, prefix_len) == 0 ? 1 : 0;
}

int string_ends_with(const char *value, const char *suffix) {
    if (value == NULL || suffix == NULL) {
        return 0;
    }
    size_t value_len = strlen(value);
    size_t suffix_len = strlen(suffix);
    if (suffix_len > value_len) {
        return 0;
    }
    return memcmp(value + value_len - suffix_len, suffix, suffix_len) == 0 ? 1 : 0;
}

const char *string_trim_start(const char *value) {
    if (value == NULL) {
        return "";
    }
    while (geo_is_space(*value)) {
        value++;
    }
    return string_clone(value);
}

const char *string_trim_end(const char *value) {
    if (value == NULL) {
        return "";
    }
    size_t len = strlen(value);
    while (len > 0 && geo_is_space(value[len - 1])) {
        len--;
    }
    return geo_string_slice(value, len);
}

const char *string_trim(const char *value) {
    if (value == NULL) {
        return "";
    }
    while (geo_is_space(*value)) {
        value++;
    }
    size_t len = strlen(value);
    while (len > 0 && geo_is_space(value[len - 1])) {
        len--;
    }
    return geo_string_slice(value, len);
}

const char *string_strip_ascii_line_comment(const char *value) {
    if (value == NULL) {
        return "";
    }
    const char *marker = strstr(value, "//");
    if (marker == NULL) {
        return string_clone(value);
    }
    return geo_string_slice(value, (size_t)(marker - value));
}

const char *string_strip_ascii_block_comment(const char *value) {
    if (value == NULL) {
        return "";
    }
    const char *start = strstr(value, "/*");
    if (start == NULL) {
        return string_clone(value);
    }
    const char *end = strstr(start + 2, "*/");
    if (end == NULL) {
        return string_clone(value);
    }
    end += 2;
    size_t prefix_len = (size_t)(start - value);
    size_t suffix_len = strlen(end);
    char *out = (char *)malloc(prefix_len + suffix_len + 1);
    if (out == NULL) {
        return "";
    }
    memcpy(out, value, prefix_len);
    memcpy(out + prefix_len, end, suffix_len);
    out[prefix_len + suffix_len] = '\0';
    return out;
}

const char *string_collapse_ascii_whitespace(const char *value) {
    if (value == NULL) {
        return "";
    }
    size_t len = strlen(value);
    char *out = (char *)malloc(len + 1);
    if (out == NULL) {
        return "";
    }
    size_t write = 0;
    int pending_space = 0;
    while (*value != '\0') {
        if (geo_is_space(*value)) {
            if (write > 0) {
                pending_space = 1;
            }
        } else {
            if (pending_space) {
                out[write++] = ' ';
                pending_space = 0;
            }
            out[write++] = *value;
        }
        value++;
    }
    out[write] = '\0';
    return out;
}

uint64_t string_line_count(const char *value) {
    if (value == NULL || value[0] == '\0') {
        return 0;
    }
    uint64_t count = 1;
    const char *cursor = value;
    while (*cursor != '\0') {
        if (*cursor == '\n' && cursor[1] != '\0') {
            count++;
        }
        cursor++;
    }
    return count;
}

const char *string_line_at(const char *value, uint64_t index) {
    if (value == NULL || value[0] == '\0') {
        return "";
    }
    uint64_t current = 0;
    const char *line_start = value;
    const char *cursor = value;
    while (1) {
        if (*cursor == '\n' || *cursor == '\0') {
            const char *line_end = cursor;
            if (line_end > line_start && line_end[-1] == '\r') {
                line_end--;
            }
            if (current == index) {
                return geo_string_slice(line_start, (size_t)(line_end - line_start));
            }
            if (*cursor == '\0') {
                break;
            }
            current++;
            line_start = cursor + 1;
        }
        cursor++;
    }
    return "";
}

const char *string_indent(const char *value, const char *prefix) {
    if (value == NULL || value[0] == '\0') {
        return "";
    }
    if (prefix == NULL) {
        prefix = "";
    }
    size_t value_len = strlen(value);
    size_t prefix_len = strlen(prefix);
    uint64_t lines = string_line_count(value);
    size_t out_len = value_len + (size_t)lines * prefix_len;
    char *out = (char *)malloc(out_len + 1);
    if (out == NULL) {
        return "";
    }
    size_t write = 0;
    int at_line_start = 1;
    for (size_t index = 0; index < value_len; index++) {
        if (at_line_start) {
            memcpy(out + write, prefix, prefix_len);
            write += prefix_len;
            at_line_start = 0;
        }
        out[write++] = value[index];
        if (value[index] == '\n' && index + 1 < value_len) {
            at_line_start = 1;
        }
    }
    out[write] = '\0';
    return out;
}

const char *string_prefix_lines(const char *value, const char *prefix) {
    if (value == NULL || value[0] == '\0') {
        return "";
    }
    if (prefix == NULL) {
        prefix = "";
    }
    size_t value_len = strlen(value);
    size_t prefix_len = strlen(prefix);
    size_t line_starts = 1;
    for (size_t index = 0; index < value_len; index++) {
        if (value[index] == '\n') {
            line_starts++;
        }
    }
    size_t out_len = value_len + line_starts * prefix_len;
    char *out = (char *)malloc(out_len + 1);
    if (out == NULL) {
        return "";
    }
    size_t write = 0;
    memcpy(out + write, prefix, prefix_len);
    write += prefix_len;
    for (size_t index = 0; index < value_len; index++) {
        out[write++] = value[index];
        if (value[index] == '\n') {
            memcpy(out + write, prefix, prefix_len);
            write += prefix_len;
        }
    }
    out[write] = '\0';
    return out;
}

const char *string_suffix_lines(const char *value, const char *suffix) {
    if (value == NULL || value[0] == '\0') {
        return "";
    }
    if (suffix == NULL) {
        suffix = "";
    }
    size_t value_len = strlen(value);
    size_t suffix_len = strlen(suffix);
    size_t line_ends = 0;
    for (size_t index = 0; index < value_len; index++) {
        if (value[index] == '\n') {
            line_ends++;
        }
    }
    if (value[value_len - 1] != '\n') {
        line_ends++;
    }
    size_t out_len = value_len + line_ends * suffix_len;
    char *out = (char *)malloc(out_len + 1);
    if (out == NULL) {
        return "";
    }
    size_t write = 0;
    for (size_t index = 0; index < value_len; index++) {
        if (value[index] == '\n') {
            memcpy(out + write, suffix, suffix_len);
            write += suffix_len;
        }
        out[write++] = value[index];
    }
    if (value[value_len - 1] != '\n') {
        memcpy(out + write, suffix, suffix_len);
        write += suffix_len;
    }
    out[write] = '\0';
    return out;
}

const char *string_dedent(const char *value, const char *prefix) {
    if (value == NULL || value[0] == '\0') {
        return "";
    }
    if (prefix == NULL || prefix[0] == '\0') {
        return string_clone(value);
    }
    size_t value_len = strlen(value);
    size_t prefix_len = strlen(prefix);
    char *out = (char *)malloc(value_len + 1);
    if (out == NULL) {
        return "";
    }
    size_t read = 0;
    size_t write = 0;
    int at_line_start = 1;
    while (read < value_len) {
        if (at_line_start && strncmp(value + read, prefix, prefix_len) == 0) {
            read += prefix_len;
        }
        at_line_start = 0;
        if (read >= value_len) {
            break;
        }
        char byte = value[read++];
        out[write++] = byte;
        if (byte == '\n' && read < value_len) {
            at_line_start = 1;
        }
    }
    out[write] = '\0';
    return out;
}

int string_line_index_at(const char *value, uint64_t offset) {
    if (value == NULL) {
        return -1;
    }
    return mem_line_index_at(value, (uint64_t)strlen(value), offset);
}

int string_column_at(const char *value, uint64_t offset) {
    if (value == NULL) {
        return -1;
    }
    return mem_column_at(value, (uint64_t)strlen(value), offset);
}

int string_offset_at_line_column(const char *value, uint64_t line_index, uint64_t column) {
    if (value == NULL) {
        return -1;
    }
    return mem_offset_at_line_column(value, (uint64_t)strlen(value), line_index, column);
}

const char *string_slice(const char *value, uint64_t start, uint64_t length) {
    if (value == NULL) {
        return "";
    }
    size_t len = strlen(value);
    if (start >= (uint64_t)len || length == 0) {
        return "";
    }
    uint64_t remaining = (uint64_t)len - start;
    uint64_t slice_len = length < remaining ? length : remaining;
    return geo_string_slice(value + start, (size_t)slice_len);
}

const char *string_utf8_slice(const char *value, uint64_t start, uint64_t end) {
    if (value == NULL || start >= end) {
        return "";
    }
    int scalar_len = string_utf8_len(value);
    if (scalar_len < 0 || start >= (uint64_t)scalar_len) {
        return "";
    }
    uint64_t clamped_end = end < (uint64_t)scalar_len ? end : (uint64_t)scalar_len;
    if (start >= clamped_end) {
        return "";
    }
    int start_offset = string_utf8_byte_offset(value, start);
    int end_offset = string_utf8_byte_offset(value, clamped_end);
    if (start_offset < 0 || end_offset < start_offset) {
        return "";
    }
    return geo_string_slice(value + start_offset, (size_t)(end_offset - start_offset));
}

const char *string_utf8_take_while_codepoint(const char *value, int codepoint) {
    if (value == NULL || codepoint <= 0 || codepoint > 0x10ffff ||
        (codepoint >= 0xd800 && codepoint <= 0xdfff)) {
        return "";
    }
    int scalar_len = string_utf8_len(value);
    if (scalar_len <= 0) {
        return "";
    }
    uint64_t end_index = 0;
    while (end_index < (uint64_t)scalar_len &&
           string_utf8_codepoint_at(value, end_index) == codepoint) {
        end_index++;
    }
    if (end_index == 0) {
        return "";
    }
    int end_offset = string_utf8_byte_offset(value, end_index);
    if (end_offset < 0) {
        return "";
    }
    return geo_string_slice(value, (size_t)end_offset);
}

const char *string_utf8_take_until_codepoint(const char *value, int codepoint) {
    if (value == NULL || codepoint <= 0 || codepoint > 0x10ffff ||
        (codepoint >= 0xd800 && codepoint <= 0xdfff)) {
        return "";
    }
    int offset = string_utf8_find_codepoint(value, codepoint);
    if (offset < 0) {
        if (!string_utf8_is_valid(value)) {
            return "";
        }
        return string_clone(value);
    }
    return geo_string_slice(value, (size_t)offset);
}

const char *string_utf8_through_codepoint(const char *value, int codepoint) {
    if (value == NULL || codepoint <= 0 || codepoint > 0x10ffff ||
        (codepoint >= 0xd800 && codepoint <= 0xdfff)) {
        return "";
    }
    int offset = string_utf8_find_codepoint(value, codepoint);
    if (offset < 0) {
        if (!string_utf8_is_valid(value)) {
            return "";
        }
        return string_clone(value);
    }
    int next_offset = string_utf8_next_offset(value, (uint64_t)offset);
    if (next_offset < 0) {
        return "";
    }
    return geo_string_slice(value, (size_t)next_offset);
}

const char *string_utf8_through_last_codepoint(const char *value,
                                               int codepoint) {
    if (value == NULL || codepoint <= 0 || codepoint > 0x10ffff ||
        (codepoint >= 0xd800 && codepoint <= 0xdfff)) {
        return "";
    }
    int offset = string_utf8_last_find_codepoint(value, codepoint);
    if (offset < 0) {
        if (!string_utf8_is_valid(value)) {
            return "";
        }
        return string_clone(value);
    }
    int next_offset = string_utf8_next_offset(value, (uint64_t)offset);
    if (next_offset < 0) {
        return "";
    }
    return geo_string_slice(value, (size_t)next_offset);
}

const char *string_utf8_between_codepoints(const char *value,
                                           int start_codepoint,
                                           int end_codepoint) {
    if (value == NULL || start_codepoint <= 0 || start_codepoint > 0x10ffff ||
        (start_codepoint >= 0xd800 && start_codepoint <= 0xdfff) ||
        end_codepoint <= 0 || end_codepoint > 0x10ffff ||
        (end_codepoint >= 0xd800 && end_codepoint <= 0xdfff)) {
        return "";
    }
    int start_offset = string_utf8_find_codepoint(value, start_codepoint);
    if (start_offset < 0) {
        return "";
    }
    int content_offset = string_utf8_next_offset(value, (uint64_t)start_offset);
    if (content_offset < 0) {
        return "";
    }
    const char *content = value + content_offset;
    int end_offset = string_utf8_find_codepoint(content, end_codepoint);
    if (end_offset < 0) {
        return "";
    }
    return geo_string_slice(content, (size_t)end_offset);
}

const char *string_utf8_between_last_codepoints(const char *value,
                                                int start_codepoint,
                                                int end_codepoint) {
    if (value == NULL || start_codepoint <= 0 || start_codepoint > 0x10ffff ||
        (start_codepoint >= 0xd800 && start_codepoint <= 0xdfff) ||
        end_codepoint <= 0 || end_codepoint > 0x10ffff ||
        (end_codepoint >= 0xd800 && end_codepoint <= 0xdfff)) {
        return "";
    }
    int start_offset = string_utf8_last_find_codepoint(value, start_codepoint);
    if (start_offset < 0) {
        return "";
    }
    int content_offset = string_utf8_next_offset(value, (uint64_t)start_offset);
    if (content_offset < 0) {
        return "";
    }
    const char *content = value + content_offset;
    int end_offset = string_utf8_find_codepoint(content, end_codepoint);
    if (end_offset < 0) {
        return "";
    }
    return geo_string_slice(content, (size_t)end_offset);
}

const char *string_utf8_between_outer_codepoints(const char *value,
                                                 int start_codepoint,
                                                 int end_codepoint) {
    if (value == NULL || start_codepoint <= 0 || start_codepoint > 0x10ffff ||
        (start_codepoint >= 0xd800 && start_codepoint <= 0xdfff) ||
        end_codepoint <= 0 || end_codepoint > 0x10ffff ||
        (end_codepoint >= 0xd800 && end_codepoint <= 0xdfff)) {
        return "";
    }
    int start_offset = string_utf8_find_codepoint(value, start_codepoint);
    if (start_offset < 0) {
        return "";
    }
    int content_offset = string_utf8_next_offset(value, (uint64_t)start_offset);
    if (content_offset < 0) {
        return "";
    }
    int end_offset = string_utf8_last_find_codepoint(value, end_codepoint);
    if (end_offset < content_offset) {
        return "";
    }
    return geo_string_slice(value + content_offset,
                            (size_t)(end_offset - content_offset));
}

const char *string_utf8_before_codepoint(const char *value, int codepoint) {
    return string_utf8_take_until_codepoint(value, codepoint);
}

const char *string_utf8_before_last_codepoint(const char *value, int codepoint) {
    if (value == NULL || codepoint <= 0 || codepoint > 0x10ffff ||
        (codepoint >= 0xd800 && codepoint <= 0xdfff)) {
        return "";
    }
    int offset = string_utf8_last_find_codepoint(value, codepoint);
    if (offset < 0) {
        if (!string_utf8_is_valid(value)) {
            return "";
        }
        return string_clone(value);
    }
    return geo_string_slice(value, (size_t)offset);
}

const char *string_utf8_drop_until_codepoint(const char *value, int codepoint) {
    if (value == NULL || codepoint <= 0 || codepoint > 0x10ffff ||
        (codepoint >= 0xd800 && codepoint <= 0xdfff)) {
        return "";
    }
    int offset = string_utf8_find_codepoint(value, codepoint);
    if (offset < 0) {
        return "";
    }
    return string_clone(value + offset);
}

const char *string_utf8_after_codepoint(const char *value, int codepoint) {
    if (value == NULL || codepoint <= 0 || codepoint > 0x10ffff ||
        (codepoint >= 0xd800 && codepoint <= 0xdfff)) {
        return "";
    }
    int offset = string_utf8_find_codepoint(value, codepoint);
    if (offset < 0) {
        return "";
    }
    int next_offset = string_utf8_next_offset(value, (uint64_t)offset);
    if (next_offset < 0) {
        return "";
    }
    return string_clone(value + next_offset);
}

const char *string_utf8_after_last_codepoint(const char *value, int codepoint) {
    if (value == NULL || codepoint <= 0 || codepoint > 0x10ffff ||
        (codepoint >= 0xd800 && codepoint <= 0xdfff)) {
        return "";
    }
    int offset = string_utf8_last_find_codepoint(value, codepoint);
    if (offset < 0) {
        return "";
    }
    int next_offset = string_utf8_next_offset(value, (uint64_t)offset);
    if (next_offset < 0) {
        return "";
    }
    return string_clone(value + next_offset);
}

const char *string_utf8_drop_while_codepoint(const char *value, int codepoint) {
    if (value == NULL) {
        return "";
    }
    if (codepoint <= 0 || codepoint > 0x10ffff ||
        (codepoint >= 0xd800 && codepoint <= 0xdfff)) {
        return string_clone(value);
    }
    int scalar_len = string_utf8_len(value);
    if (scalar_len < 0) {
        return string_clone(value);
    }
    uint64_t start_index = 0;
    while (start_index < (uint64_t)scalar_len &&
           string_utf8_codepoint_at(value, start_index) == codepoint) {
        start_index++;
    }
    int start_offset = string_utf8_byte_offset(value, start_index);
    if (start_offset < 0) {
        return string_clone(value);
    }
    return string_clone(value + start_offset);
}

const char *string_utf8_strip_prefix_codepoint(const char *value, int codepoint) {
    if (value == NULL) {
        return "";
    }
    if (!string_utf8_starts_with_codepoint(value, codepoint)) {
        return string_clone(value);
    }
    int next_offset = string_utf8_next_offset(value, 0);
    if (next_offset < 0) {
        return string_clone(value);
    }
    return string_clone(value + next_offset);
}

const char *string_utf8_strip_suffix_codepoint(const char *value, int codepoint) {
    if (value == NULL) {
        return "";
    }
    if (!string_utf8_ends_with_codepoint(value, codepoint)) {
        return string_clone(value);
    }
    int scalar_len = string_utf8_len(value);
    if (scalar_len <= 0) {
        return string_clone(value);
    }
    int end_offset = string_utf8_byte_offset(value, (uint64_t)scalar_len - 1);
    if (end_offset < 0) {
        return string_clone(value);
    }
    return geo_string_slice(value, (size_t)end_offset);
}

const char *string_utf8_trim_start_codepoint(const char *value, int codepoint) {
    if (value == NULL) {
        return "";
    }
    const char *cursor = value;
    while (string_utf8_starts_with_codepoint(cursor, codepoint)) {
        int next_offset = string_utf8_next_offset(cursor, 0);
        if (next_offset <= 0) {
            break;
        }
        cursor += next_offset;
    }
    return string_clone(cursor);
}

const char *string_utf8_trim_end_codepoint(const char *value, int codepoint) {
    if (value == NULL) {
        return "";
    }
    if (codepoint <= 0 || codepoint > 0x10ffff ||
        (codepoint >= 0xd800 && codepoint <= 0xdfff)) {
        return string_clone(value);
    }
    int scalar_len = string_utf8_len(value);
    if (scalar_len < 0) {
        return string_clone(value);
    }
    uint64_t end_index = (uint64_t)scalar_len;
    while (end_index > 0 &&
           string_utf8_codepoint_at(value, end_index - 1) == codepoint) {
        end_index--;
    }
    int end_offset = string_utf8_byte_offset(value, end_index);
    if (end_offset < 0) {
        return string_clone(value);
    }
    return geo_string_slice(value, (size_t)end_offset);
}

const char *string_utf8_trim_codepoint(const char *value, int codepoint) {
    if (value == NULL) {
        return "";
    }
    const char *start_trimmed = string_utf8_trim_start_codepoint(value, codepoint);
    return string_utf8_trim_end_codepoint(start_trimmed, codepoint);
}

const char *string_take(const char *value, uint64_t count) {
    if (value == NULL || count == 0) {
        return "";
    }
    size_t len = strlen(value);
    size_t take_len = count < (uint64_t)len ? (size_t)count : len;
    return geo_string_slice(value, take_len);
}

const char *string_drop(const char *value, uint64_t count) {
    if (value == NULL) {
        return "";
    }
    size_t len = strlen(value);
    if (count >= (uint64_t)len) {
        return "";
    }
    return geo_string_slice(value + count, len - (size_t)count);
}

const char *string_take_last(const char *value, uint64_t count) {
    if (value == NULL || count == 0) {
        return "";
    }
    size_t len = strlen(value);
    size_t take_len = count < (uint64_t)len ? (size_t)count : len;
    return geo_string_slice(value + len - take_len, take_len);
}

const char *string_drop_last(const char *value, uint64_t count) {
    if (value == NULL) {
        return "";
    }
    size_t len = strlen(value);
    if (count >= (uint64_t)len) {
        return "";
    }
    return geo_string_slice(value, len - (size_t)count);
}

const char *string_to_lower(const char *value) {
    if (value == NULL) {
        return "";
    }
    size_t len = strlen(value);
    char *out = (char *)malloc(len + 1);
    if (out == NULL) {
        return "";
    }
    for (size_t i = 0; i < len; i++) {
        char ch = value[i];
        out[i] = (ch >= 'A' && ch <= 'Z') ? (char)(ch + ('a' - 'A')) : ch;
    }
    out[len] = '\0';
    return out;
}

const char *string_to_upper(const char *value) {
    if (value == NULL) {
        return "";
    }
    size_t len = strlen(value);
    char *out = (char *)malloc(len + 1);
    if (out == NULL) {
        return "";
    }
    for (size_t i = 0; i < len; i++) {
        char ch = value[i];
        out[i] = (ch >= 'a' && ch <= 'z') ? (char)(ch - ('a' - 'A')) : ch;
    }
    out[len] = '\0';
    return out;
}

int ascii_to_lower(int byte) {
    if (byte >= 'A' && byte <= 'Z') {
        return byte + ('a' - 'A');
    }
    return byte;
}

int ascii_to_upper(int byte) {
    if (byte >= 'a' && byte <= 'z') {
        return byte - ('a' - 'A');
    }
    return byte;
}

int unicode_ascii_to_lower(int codepoint) {
    return codepoint >= 0 && codepoint <= 0x7f ? ascii_to_lower(codepoint) : codepoint;
}

int unicode_ascii_to_upper(int codepoint) {
    return codepoint >= 0 && codepoint <= 0x7f ? ascii_to_upper(codepoint) : codepoint;
}

int ascii_digit_value(int byte) {
    if (byte >= '0' && byte <= '9') {
        return byte - '0';
    }
    return -1;
}

int ascii_hex_value(int byte) {
    if (byte >= '0' && byte <= '9') {
        return byte - '0';
    }
    if (byte >= 'A' && byte <= 'F') {
        return 10 + byte - 'A';
    }
    if (byte >= 'a' && byte <= 'f') {
        return 10 + byte - 'a';
    }
    return -1;
}

int unicode_ascii_digit_value(int codepoint) {
    return codepoint >= 0 && codepoint <= 0x7f ? ascii_digit_value(codepoint) : -1;
}

int unicode_ascii_hex_value(int codepoint) {
    return codepoint >= 0 && codepoint <= 0x7f ? ascii_hex_value(codepoint) : -1;
}

int ascii_is_digit(int byte) {
    return byte >= '0' && byte <= '9' ? 1 : 0;
}

int ascii_is_hex_digit(int byte) {
    return ascii_is_digit(byte) || (byte >= 'A' && byte <= 'F') || (byte >= 'a' && byte <= 'f')
               ? 1
               : 0;
}

int ascii_is_alpha(int byte) {
    return (byte >= 'A' && byte <= 'Z') || (byte >= 'a' && byte <= 'z') ? 1 : 0;
}

int ascii_is_identifier_start(int byte) {
    return ascii_is_alpha(byte) || byte == '_' ? 1 : 0;
}

int ascii_is_identifier_continue(int byte) {
    return ascii_is_identifier_start(byte) || ascii_is_digit(byte) ? 1 : 0;
}

int unicode_is_ascii_identifier_start(int codepoint) {
    return codepoint >= 0 && codepoint <= 0x7f && ascii_is_identifier_start(codepoint) ? 1 : 0;
}

int unicode_is_ascii_identifier_continue(int codepoint) {
    return codepoint >= 0 && codepoint <= 0x7f && ascii_is_identifier_continue(codepoint) ? 1 : 0;
}

int unicode_is_ascii_digit(int codepoint) {
    return codepoint >= 0 && codepoint <= 0x7f && ascii_is_digit(codepoint) ? 1 : 0;
}

int unicode_is_ascii_hex_digit(int codepoint) {
    return codepoint >= 0 && codepoint <= 0x7f && ascii_is_hex_digit(codepoint) ? 1 : 0;
}

int unicode_is_ascii_alpha(int codepoint) {
    return codepoint >= 0 && codepoint <= 0x7f && ascii_is_alpha(codepoint) ? 1 : 0;
}

int ascii_is_alnum(int byte) {
    return ascii_is_alpha(byte) || ascii_is_digit(byte) ? 1 : 0;
}

int unicode_is_ascii_alnum(int codepoint) {
    return codepoint >= 0 && codepoint <= 0x7f && ascii_is_alnum(codepoint) ? 1 : 0;
}

int ascii_is_whitespace(int byte) {
    return byte == ' ' || byte == '\t' || byte == '\n' || byte == '\r' || byte == '\v' ||
                   byte == '\f'
               ? 1
               : 0;
}

int unicode_is_ascii_whitespace(int codepoint) {
    return codepoint >= 0 && codepoint <= 0x7f && ascii_is_whitespace(codepoint) ? 1 : 0;
}

const char *string_reverse(const char *value) {
    if (value == NULL) {
        return "";
    }
    size_t len = strlen(value);
    char *out = (char *)malloc(len + 1);
    if (out == NULL) {
        return "";
    }
    for (size_t i = 0; i < len; i++) {
        out[i] = value[len - 1 - i];
    }
    out[len] = '\0';
    return out;
}

const char *string_replace(const char *value, const char *needle, const char *replacement) {
    if (value == NULL) {
        return "";
    }
    if (needle == NULL || replacement == NULL) {
        return string_clone(value);
    }
    size_t needle_len = strlen(needle);
    if (needle_len == 0) {
        return string_clone(value);
    }
    const char *match = strstr(value, needle);
    if (match == NULL) {
        return string_clone(value);
    }

    size_t value_len = strlen(value);
    size_t replacement_len = strlen(replacement);
    size_t prefix_len = (size_t)(match - value);
    size_t suffix_len = value_len - prefix_len - needle_len;
    char *out = (char *)malloc(prefix_len + replacement_len + suffix_len + 1);
    if (out == NULL) {
        return "";
    }
    memcpy(out, value, prefix_len);
    memcpy(out + prefix_len, replacement, replacement_len);
    memcpy(out + prefix_len + replacement_len, match + needle_len, suffix_len);
    out[prefix_len + replacement_len + suffix_len] = '\0';
    return out;
}

const char *string_replace_all(const char *value, const char *needle, const char *replacement) {
    if (value == NULL) {
        return "";
    }
    if (needle == NULL || replacement == NULL) {
        return string_clone(value);
    }
    size_t needle_len = strlen(needle);
    if (needle_len == 0) {
        return string_clone(value);
    }

    size_t value_len = strlen(value);
    size_t replacement_len = strlen(replacement);
    size_t matches = 0;
    const char *cursor = value;
    while ((cursor = strstr(cursor, needle)) != NULL) {
        matches++;
        cursor += needle_len;
    }
    if (matches == 0) {
        return string_clone(value);
    }

    size_t out_len = value_len;
    if (replacement_len >= needle_len) {
        out_len += matches * (replacement_len - needle_len);
    } else {
        out_len -= matches * (needle_len - replacement_len);
    }
    char *out = (char *)malloc(out_len + 1);
    if (out == NULL) {
        return "";
    }

    const char *src = value;
    char *dst = out;
    while ((cursor = strstr(src, needle)) != NULL) {
        size_t prefix_len = (size_t)(cursor - src);
        memcpy(dst, src, prefix_len);
        dst += prefix_len;
        memcpy(dst, replacement, replacement_len);
        dst += replacement_len;
        src = cursor + needle_len;
    }
    strcpy(dst, src);
    return out;
}

const char *string_escape(const char *value) {
    if (value == NULL || value[0] == '\0') {
        return "";
    }
    size_t out_len = 0;
    const unsigned char *cursor = (const unsigned char *)value;
    while (*cursor != '\0') {
        switch (*cursor) {
        case '\n':
        case '\r':
        case '\t':
        case '\\':
        case '"':
            out_len += 2;
            break;
        default:
            out_len += 1;
            break;
        }
        cursor++;
    }
    char *out = (char *)malloc(out_len + 1);
    if (out == NULL) {
        return "";
    }
    cursor = (const unsigned char *)value;
    char *dst = out;
    while (*cursor != '\0') {
        switch (*cursor) {
        case '\n':
            *dst++ = '\\';
            *dst++ = 'n';
            break;
        case '\r':
            *dst++ = '\\';
            *dst++ = 'r';
            break;
        case '\t':
            *dst++ = '\\';
            *dst++ = 't';
            break;
        case '\\':
            *dst++ = '\\';
            *dst++ = '\\';
            break;
        case '"':
            *dst++ = '\\';
            *dst++ = '"';
            break;
        default:
            *dst++ = (char)*cursor;
            break;
        }
        cursor++;
    }
    *dst = '\0';
    return out;
}

static char geo_hex_upper_digit(unsigned char value) {
    return (char)(value < 10 ? '0' + value : 'A' + (value - 10));
}

const char *string_escape_ascii(const char *value) {
    if (value == NULL || value[0] == '\0') {
        return "";
    }
    size_t out_len = 0;
    const unsigned char *cursor = (const unsigned char *)value;
    while (*cursor != '\0') {
        unsigned char byte = *cursor;
        if (byte == '\n' || byte == '\r' || byte == '\t' || byte == '\\' || byte == '"') {
            out_len += 2;
        } else if (byte < 0x20 || byte > 0x7e) {
            out_len += 4;
        } else {
            out_len += 1;
        }
        cursor++;
    }
    char *out = (char *)malloc(out_len + 1);
    if (out == NULL) {
        return "";
    }
    cursor = (const unsigned char *)value;
    char *dst = out;
    while (*cursor != '\0') {
        unsigned char byte = *cursor++;
        switch (byte) {
        case '\n':
            *dst++ = '\\';
            *dst++ = 'n';
            break;
        case '\r':
            *dst++ = '\\';
            *dst++ = 'r';
            break;
        case '\t':
            *dst++ = '\\';
            *dst++ = 't';
            break;
        case '\\':
            *dst++ = '\\';
            *dst++ = '\\';
            break;
        case '"':
            *dst++ = '\\';
            *dst++ = '"';
            break;
        default:
            if (byte < 0x20 || byte > 0x7e) {
                *dst++ = '\\';
                *dst++ = 'x';
                *dst++ = geo_hex_upper_digit(byte >> 4);
                *dst++ = geo_hex_upper_digit(byte & 0x0f);
            } else {
                *dst++ = (char)byte;
            }
            break;
        }
    }
    *dst = '\0';
    return out;
}

static int geo_hex_digit_value(unsigned char byte) {
    if (byte >= '0' && byte <= '9') {
        return byte - '0';
    }
    if (byte >= 'A' && byte <= 'F') {
        return 10 + byte - 'A';
    }
    if (byte >= 'a' && byte <= 'f') {
        return 10 + byte - 'a';
    }
    return -1;
}

static int geo_encode_utf8(uint32_t value, char *dst) {
    if (value > 0x10ffff || (value >= 0xd800 && value <= 0xdfff)) {
        return 0;
    }
    if (value <= 0x7f) {
        dst[0] = (char)value;
        return 1;
    }
    if (value <= 0x7ff) {
        dst[0] = (char)(0xc0 | (value >> 6));
        dst[1] = (char)(0x80 | (value & 0x3f));
        return 2;
    }
    if (value <= 0xffff) {
        dst[0] = (char)(0xe0 | (value >> 12));
        dst[1] = (char)(0x80 | ((value >> 6) & 0x3f));
        dst[2] = (char)(0x80 | (value & 0x3f));
        return 3;
    }
    dst[0] = (char)(0xf0 | (value >> 18));
    dst[1] = (char)(0x80 | ((value >> 12) & 0x3f));
    dst[2] = (char)(0x80 | ((value >> 6) & 0x3f));
    dst[3] = (char)(0x80 | (value & 0x3f));
    return 4;
}

static int geo_parse_unicode_escape(const unsigned char *src, uint32_t *out_value, size_t *out_used) {
    if (src[0] != 'u' || src[1] != '{') {
        return 0;
    }
    uint32_t value = 0;
    size_t digits = 0;
    size_t index = 2;
    while (src[index] != '\0' && src[index] != '}') {
        int digit = geo_hex_digit_value(src[index]);
        if (digit < 0 || digits == 6) {
            return 0;
        }
        value = (value << 4) | (uint32_t)digit;
        digits++;
        index++;
    }
    if (digits == 0 || src[index] != '}') {
        return 0;
    }
    char scratch[4];
    if (geo_encode_utf8(value, scratch) == 0) {
        return 0;
    }
    *out_value = value;
    *out_used = index + 1;
    return 1;
}

const char *string_unescape(const char *value) {
    if (value == NULL || value[0] == '\0') {
        return "";
    }
    size_t len = strlen(value);
    char *out = (char *)malloc(len + 1);
    if (out == NULL) {
        return "";
    }
    const unsigned char *src = (const unsigned char *)value;
    char *dst = out;
    while (*src != '\0') {
        if (*src != '\\') {
            *dst++ = (char)*src++;
            continue;
        }
        src++;
        switch (*src) {
        case '\0':
            break;
        case 'n':
            *dst++ = '\n';
            src++;
            break;
        case 'r':
            *dst++ = '\r';
            src++;
            break;
        case 't':
            *dst++ = '\t';
            src++;
            break;
        case '\\':
            *dst++ = '\\';
            src++;
            break;
        case '"':
            *dst++ = '"';
            src++;
            break;
        case 'x': {
            int high = geo_hex_digit_value(src[1]);
            int low = geo_hex_digit_value(src[2]);
            if (high >= 0 && low >= 0) {
                *dst++ = (char)((high << 4) | low);
                src += 3;
            } else {
                *dst++ = (char)*src++;
            }
            break;
        }
        case 'u': {
            uint32_t scalar = 0;
            size_t used = 0;
            if (geo_parse_unicode_escape(src, &scalar, &used)) {
                dst += geo_encode_utf8(scalar, dst);
                src += used;
            } else {
                *dst++ = (char)*src++;
            }
            break;
        }
        default:
            *dst++ = (char)*src++;
            break;
        }
    }
    *dst = '\0';
    return out;
}

const char *string_unescape_hex(const char *value) {
    return string_unescape(value);
}

const char *string_unescape_unicode(const char *value) {
    return string_unescape(value);
}

const char *string_repeat(const char *value, uint64_t count) {
    if (value == NULL || count == 0) {
        return "";
    }
    size_t value_len = strlen(value);
    if (value_len == 0) {
        return "";
    }
    if (count > UINT64_MAX / (uint64_t)value_len) {
        return "";
    }
    uint64_t out_len64 = (uint64_t)value_len * count;
    if (out_len64 > (uint64_t)SIZE_MAX - 1u) {
        return "";
    }
    size_t out_len = (size_t)out_len64;
    char *out = (char *)malloc(out_len + 1);
    if (out == NULL) {
        return "";
    }
    char *cursor = out;
    for (uint64_t i = 0; i < count; i++) {
        memcpy(cursor, value, value_len);
        cursor += value_len;
    }
    out[out_len] = '\0';
    return out;
}

static const char *geo_string_pad(const char *value, uint64_t width, const char *pad, int at_start) {
    if (value == NULL) {
        return "";
    }
    if (pad == NULL || pad[0] == '\0') {
        return string_clone(value);
    }
    size_t value_len = strlen(value);
    if (width <= (uint64_t)value_len) {
        return string_clone(value);
    }
    uint64_t pad_count64 = width - (uint64_t)value_len;
    if (pad_count64 > (uint64_t)SIZE_MAX - value_len - 1u) {
        return "";
    }
    size_t pad_count = (size_t)pad_count64;
    size_t out_len = value_len + pad_count;
    char *out = (char *)malloc(out_len + 1);
    if (out == NULL) {
        return "";
    }
    if (at_start) {
        memset(out, pad[0], pad_count);
        memcpy(out + pad_count, value, value_len);
    } else {
        memcpy(out, value, value_len);
        memset(out + value_len, pad[0], pad_count);
    }
    out[out_len] = '\0';
    return out;
}

const char *string_pad_start(const char *value, uint64_t width, const char *pad) {
    return geo_string_pad(value, width, pad, 1);
}

const char *string_pad_end(const char *value, uint64_t width, const char *pad) {
    return geo_string_pad(value, width, pad, 0);
}

int string_parse_int(const char *value) {
    if (value == NULL) {
        return 0;
    }
    char *end = NULL;
    long parsed = strtol(value, &end, 10);
    if (end == value) {
        return 0;
    }
    return (int)parsed;
}

uint64_t string_parse_usize(const char *value) {
    if (value == NULL) {
        return 0;
    }
    while (geo_is_space(*value)) {
        value++;
    }
    if (*value == '-') {
        return 0;
    }
    char *end = NULL;
    unsigned long long parsed = strtoull(value, &end, 10);
    if (end == value) {
        return 0;
    }
    return (uint64_t)parsed;
}

static int geo_parse_consumed_all(const char *cursor) {
    while (cursor != NULL && geo_is_space(*cursor)) {
        cursor++;
    }
    return cursor != NULL && *cursor == '\0';
}

int string_try_parse_int(const char *value, int64_t *out) {
    if (value == NULL || out == NULL) {
        return 0;
    }
    errno = 0;
    char *end = NULL;
    long long parsed = geo_strtoi64(value, &end, 10);
    if (end == value || errno == ERANGE || !geo_parse_consumed_all(end)) {
        return 0;
    }
    *out = (int64_t)parsed;
    return 1;
}

int string_try_parse_usize(const char *value, uint64_t *out) {
    if (value == NULL || out == NULL) {
        return 0;
    }
    while (geo_is_space(*value)) {
        value++;
    }
    if (*value == '-') {
        return 0;
    }
    errno = 0;
    char *end = NULL;
    unsigned long long parsed = geo_strtou64(value, &end, 10);
    if (end == value || errno == ERANGE || !geo_parse_consumed_all(end)) {
        return 0;
    }
    *out = (uint64_t)parsed;
    return 1;
}

const char *int_to_string(int64_t value) {
    int len = snprintf(NULL, 0, "%lld", (long long)value);
    if (len < 0) {
        return "";
    }
    char *out = (char *)malloc((size_t)len + 1);
    if (out == NULL) {
        return "";
    }
    snprintf(out, (size_t)len + 1, "%lld", (long long)value);
    return out;
}

const char *usize_to_string(uint64_t value) {
    int len = snprintf(NULL, 0, "%llu", (unsigned long long)value);
    if (len < 0) {
        return "";
    }
    char *out = (char *)malloc((size_t)len + 1);
    if (out == NULL) {
        return "";
    }
    snprintf(out, (size_t)len + 1, "%llu", (unsigned long long)value);
    return out;
}

const char *bool_to_string(int value) {
    return value ? "true" : "false";
}

const char *string_concat(const char *left, const char *right) {
    if (left == NULL) {
        left = "";
    }
    if (right == NULL) {
        right = "";
    }
    size_t left_len = strlen(left);
    size_t right_len = strlen(right);
    char *value = (char *)malloc(left_len + right_len + 1);
    if (value == NULL) {
        return NULL;
    }
    memcpy(value, left, left_len);
    memcpy(value + left_len, right, right_len + 1);
    return value;
}

const char *substring(const char *value, uint64_t start, uint64_t len) {
    if (value == NULL) {
        return NULL;
    }
    size_t value_len = strlen(value);
    if (start > value_len) {
        start = value_len;
    }
    if (start + len > value_len) {
        len = value_len - start;
    }
    char *out = (char *)malloc((size_t)len + 1);
    if (out == NULL) {
        return NULL;
    }
    memcpy(out, value + start, (size_t)len);
    out[len] = '\0';
    return out;
}

void *array_new(uint64_t element_size, uint64_t capacity) {
    if (element_size == 0) {
        return NULL;
    }
    if (capacity > (UINT64_MAX - sizeof(GeoArrayHeader)) / element_size) {
        return NULL;
    }
    GeoArrayHeader *array = (GeoArrayHeader *)malloc(sizeof(GeoArrayHeader) + (size_t)(element_size * capacity));
    if (array == NULL) {
        return NULL;
    }
    array->len = 0;
    array->cap = capacity;
    array->elem_size = element_size;
    return array;
}

void *array_clone(void *ptr) {
    GeoArrayHeader *array = geo_array_from_ptr(ptr);
    if (array == NULL) {
        return NULL;
    }
    if (array->cap > (UINT64_MAX - sizeof(GeoArrayHeader)) / array->elem_size) {
        return NULL;
    }
    GeoArrayHeader *copy = (GeoArrayHeader *)malloc(sizeof(GeoArrayHeader) + (size_t)(array->elem_size * array->cap));
    if (copy == NULL) {
        return NULL;
    }
    copy->len = array->len;
    copy->cap = array->cap;
    copy->elem_size = array->elem_size;
    if (array->len > 0) {
        memcpy(copy->data, array->data, (size_t)(array->elem_size * array->len));
    }
    return copy;
}

void *array_reserve(void *ptr, uint64_t capacity) {
    GeoArrayHeader *array = geo_array_from_ptr(ptr);
    if (array == NULL) {
        return NULL;
    }
    if (capacity <= array->cap) {
        return ptr;
    }
    if (capacity > (UINT64_MAX - sizeof(GeoArrayHeader)) / array->elem_size) {
        return NULL;
    }
    GeoArrayHeader *grown = (GeoArrayHeader *)realloc(array, sizeof(GeoArrayHeader) + (size_t)(array->elem_size * capacity));
    if (grown == NULL) {
        return NULL;
    }
    grown->cap = capacity;
    return grown;
}

uint64_t array_len(void *ptr) {
    GeoArrayHeader *array = geo_array_from_ptr(ptr);
    return array == NULL ? 0 : array->len;
}

int array_is_empty(void *ptr) {
    GeoArrayHeader *array = geo_array_from_ptr(ptr);
    return array == NULL || array->len == 0;
}

uint64_t array_capacity(void *ptr) {
    GeoArrayHeader *array = geo_array_from_ptr(ptr);
    return array == NULL ? 0 : array->cap;
}

int array_push(void *ptr, void *value) {
    GeoArrayHeader *array = geo_array_from_ptr(ptr);
    if (array == NULL || value == NULL || array->len >= array->cap) {
        return 1;
    }
    memcpy(array->data + (size_t)(array->len * array->elem_size), value, (size_t)array->elem_size);
    array->len++;
    return 0;
}

void *array_get(void *ptr, uint64_t index) {
    GeoArrayHeader *array = geo_array_from_ptr(ptr);
    if (array == NULL || index >= array->len) {
        return NULL;
    }
    return array->data + (size_t)(index * array->elem_size);
}

void *array_first(void *ptr) {
    GeoArrayHeader *array = geo_array_from_ptr(ptr);
    if (array == NULL || array->len == 0) {
        return NULL;
    }
    return array->data;
}

void *array_last(void *ptr) {
    GeoArrayHeader *array = geo_array_from_ptr(ptr);
    if (array == NULL || array->len == 0) {
        return NULL;
    }
    return array->data + (size_t)((array->len - 1) * array->elem_size);
}

int array_index_of(void *ptr, void *value) {
    GeoArrayHeader *array = geo_array_from_ptr(ptr);
    if (array == NULL || value == NULL) {
        return -1;
    }
    for (uint64_t index = 0; index < array->len; index++) {
        unsigned char *item = array->data + (size_t)(index * array->elem_size);
        if (memcmp(item, value, (size_t)array->elem_size) == 0) {
            if (index > (uint64_t)INT32_MAX) {
                return -1;
            }
            return (int)index;
        }
    }
    return -1;
}

int array_last_index_of(void *ptr, void *value) {
    GeoArrayHeader *array = geo_array_from_ptr(ptr);
    if (array == NULL || value == NULL) {
        return -1;
    }
    uint64_t index = array->len;
    while (index > 0) {
        index--;
        unsigned char *item = array->data + (size_t)(index * array->elem_size);
        if (memcmp(item, value, (size_t)array->elem_size) == 0) {
            if (index > (uint64_t)INT32_MAX) {
                return -1;
            }
            return (int)index;
        }
    }
    return -1;
}

int array_contains(void *ptr, void *value) {
    GeoArrayHeader *array = geo_array_from_ptr(ptr);
    if (array == NULL || value == NULL) {
        return 0;
    }
    for (uint64_t index = 0; index < array->len; index++) {
        unsigned char *item = array->data + (size_t)(index * array->elem_size);
        if (memcmp(item, value, (size_t)array->elem_size) == 0) {
            return 1;
        }
    }
    return 0;
}

uint64_t array_count(void *ptr, void *value) {
    GeoArrayHeader *array = geo_array_from_ptr(ptr);
    if (array == NULL || value == NULL) {
        return 0;
    }
    uint64_t count = 0;
    for (uint64_t index = 0; index < array->len; index++) {
        unsigned char *item = array->data + (size_t)(index * array->elem_size);
        if (memcmp(item, value, (size_t)array->elem_size) == 0) {
            count++;
        }
    }
    return count;
}

int array_set(void *ptr, uint64_t index, void *value) {
    GeoArrayHeader *array = geo_array_from_ptr(ptr);
    if (array == NULL || value == NULL || index >= array->len) {
        return 1;
    }
    memcpy(array->data + (size_t)(index * array->elem_size), value, (size_t)array->elem_size);
    return 0;
}

int array_fill(void *ptr, void *value) {
    GeoArrayHeader *array = geo_array_from_ptr(ptr);
    if (array == NULL || value == NULL) {
        return 1;
    }
    for (uint64_t index = 0; index < array->len; index++) {
        memcpy(array->data + (size_t)(index * array->elem_size), value, (size_t)array->elem_size);
    }
    return 0;
}

int array_extend(void *ptr, void *other) {
    GeoArrayHeader *array = geo_array_from_ptr(ptr);
    GeoArrayHeader *source = geo_array_from_ptr(other);
    if (array == NULL || source == NULL || array->elem_size != source->elem_size) {
        return 1;
    }
    if (source->len > array->cap - array->len) {
        return 1;
    }
    if (source->len > 0) {
        memmove(
            array->data + (size_t)(array->len * array->elem_size),
            source->data,
            (size_t)(source->len * source->elem_size));
    }
    array->len += source->len;
    return 0;
}

int array_copy(void *dst, uint64_t dst_index, void *src, uint64_t src_index, uint64_t count) {
    GeoArrayHeader *target = geo_array_from_ptr(dst);
    GeoArrayHeader *source = geo_array_from_ptr(src);
    if (target == NULL || source == NULL || target->elem_size != source->elem_size) {
        return 1;
    }
    if (count == 0) {
        return 0;
    }
    if (dst_index > target->len || src_index > source->len) {
        return 1;
    }
    if (count > target->len - dst_index || count > source->len - src_index) {
        return 1;
    }
    memmove(
        target->data + (size_t)(dst_index * target->elem_size),
        source->data + (size_t)(src_index * source->elem_size),
        (size_t)(count * target->elem_size));
    return 0;
}

int array_resize(void *ptr, uint64_t len, void *value) {
    GeoArrayHeader *array = geo_array_from_ptr(ptr);
    if (array == NULL || value == NULL || len > array->cap) {
        return 1;
    }
    while (array->len < len) {
        memcpy(array->data + (size_t)(array->len * array->elem_size), value, (size_t)array->elem_size);
        array->len++;
    }
    array->len = len;
    return 0;
}

int array_insert(void *ptr, uint64_t index, void *value) {
    GeoArrayHeader *array = geo_array_from_ptr(ptr);
    if (array == NULL || value == NULL || index > array->len || array->len >= array->cap) {
        return 1;
    }
    uint64_t tail = array->len - index;
    if (tail > 0) {
        memmove(
            array->data + (size_t)((index + 1) * array->elem_size),
            array->data + (size_t)(index * array->elem_size),
            (size_t)(tail * array->elem_size));
    }
    memcpy(array->data + (size_t)(index * array->elem_size), value, (size_t)array->elem_size);
    array->len++;
    return 0;
}

int array_swap(void *ptr, uint64_t left, uint64_t right) {
    GeoArrayHeader *array = geo_array_from_ptr(ptr);
    if (array == NULL || left >= array->len || right >= array->len) {
        return 1;
    }
    if (left == right) {
        return 0;
    }
    unsigned char *left_ptr = array->data + (size_t)(left * array->elem_size);
    unsigned char *right_ptr = array->data + (size_t)(right * array->elem_size);
    unsigned char *tmp = (unsigned char *)malloc((size_t)array->elem_size);
    if (tmp == NULL) {
        return 1;
    }
    memcpy(tmp, left_ptr, (size_t)array->elem_size);
    memcpy(left_ptr, right_ptr, (size_t)array->elem_size);
    memcpy(right_ptr, tmp, (size_t)array->elem_size);
    free(tmp);
    return 0;
}

int array_reverse(void *ptr) {
    GeoArrayHeader *array = geo_array_from_ptr(ptr);
    if (array == NULL) {
        return 1;
    }
    if (array->len < 2) {
        return 0;
    }
    unsigned char *tmp = (unsigned char *)malloc((size_t)array->elem_size);
    if (tmp == NULL) {
        return 1;
    }
    uint64_t left = 0;
    uint64_t right = array->len - 1;
    while (left < right) {
        unsigned char *left_ptr = array->data + (size_t)(left * array->elem_size);
        unsigned char *right_ptr = array->data + (size_t)(right * array->elem_size);
        memcpy(tmp, left_ptr, (size_t)array->elem_size);
        memcpy(left_ptr, right_ptr, (size_t)array->elem_size);
        memcpy(right_ptr, tmp, (size_t)array->elem_size);
        left++;
        right--;
    }
    free(tmp);
    return 0;
}

int array_truncate(void *ptr, uint64_t len) {
    GeoArrayHeader *array = geo_array_from_ptr(ptr);
    if (array == NULL || len > array->len) {
        return 1;
    }
    array->len = len;
    return 0;
}

int array_remove(void *ptr, uint64_t index) {
    GeoArrayHeader *array = geo_array_from_ptr(ptr);
    if (array == NULL || index >= array->len) {
        return 1;
    }
    uint64_t tail = array->len - index - 1;
    if (tail > 0) {
        memmove(
            array->data + (size_t)(index * array->elem_size),
            array->data + (size_t)((index + 1) * array->elem_size),
            (size_t)(tail * array->elem_size));
    }
    array->len--;
    return 0;
}

int array_swap_remove(void *ptr, uint64_t index) {
    GeoArrayHeader *array = geo_array_from_ptr(ptr);
    if (array == NULL || index >= array->len) {
        return 1;
    }
    uint64_t last = array->len - 1;
    if (index != last) {
        memcpy(
            array->data + (size_t)(index * array->elem_size),
            array->data + (size_t)(last * array->elem_size),
            (size_t)array->elem_size);
    }
    array->len--;
    return 0;
}

void *array_pop(void *ptr) {
    GeoArrayHeader *array = geo_array_from_ptr(ptr);
    if (array == NULL || array->len == 0) {
        return NULL;
    }
    array->len--;
    return array->data + (size_t)(array->len * array->elem_size);
}

void *array_pop_first(void *ptr) {
    GeoArrayHeader *array = geo_array_from_ptr(ptr);
    if (array == NULL || array->len == 0) {
        return NULL;
    }
    unsigned char *first = array->data;
    unsigned char *out = array->data + (size_t)((array->len - 1) * array->elem_size);
    if (array->len > 1) {
        memcpy(out, first, (size_t)array->elem_size);
        memmove(first, first + array->elem_size, (size_t)((array->len - 1) * array->elem_size));
    }
    array->len--;
    return out;
}

int array_clear(void *ptr) {
    GeoArrayHeader *array = geo_array_from_ptr(ptr);
    if (array == NULL) {
        return 1;
    }
    array->len = 0;
    return 0;
}

int array_free(void *ptr) {
    free(ptr);
    return 0;
}

int64_t int_abs(int64_t value) {
    return value < 0 ? -value : value;
}

uint64_t int_abs_diff(int64_t left, int64_t right) {
    uint64_t left_magnitude = left < 0 ? 0 - (uint64_t)left : (uint64_t)left;
    uint64_t right_magnitude = right < 0 ? 0 - (uint64_t)right : (uint64_t)right;
    if ((left < 0) != (right < 0)) {
        return left_magnitude + right_magnitude;
    }
    return left_magnitude > right_magnitude ? left_magnitude - right_magnitude
                                             : right_magnitude - left_magnitude;
}

int64_t int_min(int64_t left, int64_t right) {
    return left < right ? left : right;
}

int64_t int_max(int64_t left, int64_t right) {
    return left > right ? left : right;
}

int64_t int_clamp(int64_t value, int64_t min, int64_t max) {
    if (min > max) {
        int64_t temp = min;
        min = max;
        max = temp;
    }
    if (value < min) {
        return min;
    }
    if (value > max) {
        return max;
    }
    return value;
}

int64_t int_div_floor(int64_t left, int64_t right) {
    if (right == 0) {
        return 0;
    }
    int64_t quotient = left / right;
    int64_t remainder = left % right;
    if (remainder != 0 && ((left < 0) != (right < 0))) {
        quotient--;
    }
    return quotient;
}

int64_t int_div_ceil(int64_t left, int64_t right) {
    if (right == 0) {
        return 0;
    }
    int64_t quotient = left / right;
    int64_t remainder = left % right;
    if (remainder != 0 && ((left < 0) == (right < 0))) {
        quotient++;
    }
    return quotient;
}

int64_t int_div_euclid(int64_t left, int64_t right) {
    if (right == 0) {
        return 0;
    }
    int64_t modulus = right < 0 ? -right : right;
    int64_t remainder = left % modulus;
    if (remainder < 0) {
        remainder += modulus;
    }
    return (left - remainder) / right;
}

int64_t int_rem_floor(int64_t left, int64_t right) {
    if (right == 0) {
        return 0;
    }
    int64_t remainder = left % right;
    if (remainder != 0 && ((left < 0) != (right < 0))) {
        remainder += right;
    }
    return remainder;
}

int64_t int_rem_euclid(int64_t left, int64_t right) {
    if (right == 0) {
        return 0;
    }
    int64_t modulus = right < 0 ? -right : right;
    int64_t remainder = left % modulus;
    return remainder < 0 ? remainder + modulus : remainder;
}

int64_t int_checked_add(int64_t left, int64_t right) {
    if ((right > 0 && left > INT64_MAX - right) || (right < 0 && left < INT64_MIN - right)) {
        return 0;
    }
    return left + right;
}

int64_t int_checked_sub(int64_t left, int64_t right) {
    if ((right > 0 && left < INT64_MIN + right) || (right < 0 && left > INT64_MAX + right)) {
        return 0;
    }
    return left - right;
}

int64_t int_checked_mul(int64_t left, int64_t right) {
    if (left == 0 || right == 0) {
        return 0;
    }
    if (left > 0) {
        if ((right > 0 && left > INT64_MAX / right) || (right < 0 && right < INT64_MIN / left)) {
            return 0;
        }
    } else if ((right > 0 && left < INT64_MIN / right) || (right < 0 && left < INT64_MAX / right)) {
        return 0;
    }
    return left * right;
}

int64_t int_checked_div(int64_t left, int64_t right) {
    if (right == 0 || (left == INT64_MIN && right == -1)) {
        return 0;
    }
    return left / right;
}

int64_t int_checked_rem(int64_t left, int64_t right) {
    if (right == 0 || (left == INT64_MIN && right == -1)) {
        return 0;
    }
    return left % right;
}

int64_t int_checked_neg(int64_t value) {
    if (value == INT64_MIN) {
        return 0;
    }
    return -value;
}

int64_t int_checked_abs(int64_t value) {
    if (value == INT64_MIN) {
        return 0;
    }
    return value < 0 ? -value : value;
}

int64_t int_saturating_add(int64_t left, int64_t right) {
    if (right > 0 && left > INT64_MAX - right) {
        return INT64_MAX;
    }
    if (right < 0 && left < INT64_MIN - right) {
        return INT64_MIN;
    }
    return left + right;
}

int64_t int_saturating_sub(int64_t left, int64_t right) {
    if (right < 0 && left > INT64_MAX + right) {
        return INT64_MAX;
    }
    if (right > 0 && left < INT64_MIN + right) {
        return INT64_MIN;
    }
    return left - right;
}

int64_t int_saturating_mul(int64_t left, int64_t right) {
    if (left == 0 || right == 0) {
        return 0;
    }
    if (left > 0) {
        if (right > 0 && left > INT64_MAX / right) {
            return INT64_MAX;
        }
        if (right < 0 && right < INT64_MIN / left) {
            return INT64_MIN;
        }
    } else {
        if (right > 0 && left < INT64_MIN / right) {
            return INT64_MIN;
        }
        if (right < 0 && left < INT64_MAX / right) {
            return INT64_MAX;
        }
    }
    return left * right;
}

int64_t int_saturating_abs(int64_t value) {
    if (value == INT64_MIN) {
        return INT64_MAX;
    }
    return value < 0 ? -value : value;
}

int64_t int_saturating_neg(int64_t value) {
    if (value == INT64_MIN) {
        return INT64_MAX;
    }
    return -value;
}

int64_t int_wrapping_add(int64_t left, int64_t right) {
    uint64_t result = (uint64_t)left + (uint64_t)right;
    return (int64_t)result;
}

int64_t int_wrapping_sub(int64_t left, int64_t right) {
    uint64_t result = (uint64_t)left - (uint64_t)right;
    return (int64_t)result;
}

int64_t int_wrapping_mul(int64_t left, int64_t right) {
    uint64_t result = (uint64_t)left * (uint64_t)right;
    return (int64_t)result;
}

int64_t int_wrapping_neg(int64_t value) {
    uint64_t result = 0 - (uint64_t)value;
    return (int64_t)result;
}

int64_t int_wrapping_abs(int64_t value) {
    return value < 0 ? int_wrapping_neg(value) : value;
}

uint64_t usize_min(uint64_t left, uint64_t right) {
    return left < right ? left : right;
}

uint64_t usize_max(uint64_t left, uint64_t right) {
    return left > right ? left : right;
}

uint64_t usize_clamp(uint64_t value, uint64_t min, uint64_t max) {
    if (min > max) {
        uint64_t temp = min;
        min = max;
        max = temp;
    }
    if (value < min) {
        return min;
    }
    if (value > max) {
        return max;
    }
    return value;
}

uint64_t usize_abs_diff(uint64_t left, uint64_t right) {
    return left > right ? left - right : right - left;
}

uint64_t usize_checked_add(uint64_t left, uint64_t right) {
    if (left > UINT64_MAX - right) {
        return 0;
    }
    return left + right;
}

uint64_t usize_checked_sub(uint64_t left, uint64_t right) {
    if (left < right) {
        return 0;
    }
    return left - right;
}

uint64_t usize_checked_mul(uint64_t left, uint64_t right) {
    if (left != 0 && right > UINT64_MAX / left) {
        return 0;
    }
    return left * right;
}

uint64_t usize_checked_div(uint64_t left, uint64_t right) {
    if (right == 0) {
        return 0;
    }
    return left / right;
}

uint64_t usize_checked_rem(uint64_t left, uint64_t right) {
    if (right == 0) {
        return 0;
    }
    return left % right;
}

uint64_t usize_saturating_add(uint64_t left, uint64_t right) {
    if (left > UINT64_MAX - right) {
        return UINT64_MAX;
    }
    return left + right;
}

uint64_t usize_saturating_sub(uint64_t left, uint64_t right) {
    if (left < right) {
        return 0;
    }
    return left - right;
}

uint64_t usize_saturating_mul(uint64_t left, uint64_t right) {
    if (left != 0 && right > UINT64_MAX / left) {
        return UINT64_MAX;
    }
    return left * right;
}

uint64_t usize_wrapping_add(uint64_t left, uint64_t right) {
    return left + right;
}

uint64_t usize_wrapping_sub(uint64_t left, uint64_t right) {
    return left - right;
}

uint64_t usize_wrapping_mul(uint64_t left, uint64_t right) {
    return left * right;
}

int64_t int_pow(int64_t base, uint64_t exp) {
    int64_t result = 1;
    while (exp > 0) {
        if ((exp & 1ULL) != 0) {
            result *= base;
        }
        exp >>= 1;
        if (exp > 0) {
            base *= base;
        }
    }
    return result;
}

int64_t int_checked_pow(int64_t base, uint64_t exp) {
    int64_t result = 1;
    while (exp > 0) {
        if ((exp & 1ULL) != 0) {
            if (result != 0 && base != 0 && int_checked_mul(result, base) == 0) {
                return 0;
            }
            result *= base;
        }
        exp >>= 1;
        if (exp > 0) {
            if (base != 0 && int_checked_mul(base, base) == 0) {
                return 0;
            }
            base *= base;
        }
    }
    return result;
}

int64_t int_saturating_pow(int64_t base, uint64_t exp) {
    int64_t result = 1;
    while (exp > 0) {
        if ((exp & 1ULL) != 0) {
            result = int_saturating_mul(result, base);
        }
        exp >>= 1;
        if (exp > 0) {
            base = int_saturating_mul(base, base);
        }
    }
    return result;
}

int64_t int_wrapping_pow(int64_t base, uint64_t exp) {
    int64_t result = 1;
    while (exp > 0) {
        if ((exp & 1ULL) != 0) {
            result = int_wrapping_mul(result, base);
        }
        exp >>= 1;
        if (exp > 0) {
            base = int_wrapping_mul(base, base);
        }
    }
    return result;
}

uint64_t usize_pow(uint64_t base, uint64_t exp) {
    uint64_t result = 1;
    while (exp > 0) {
        if ((exp & 1ULL) != 0) {
            result *= base;
        }
        exp >>= 1;
        if (exp > 0) {
            base *= base;
        }
    }
    return result;
}

uint64_t usize_wrapping_pow(uint64_t base, uint64_t exp) {
    uint64_t result = 1;
    while (exp > 0) {
        if ((exp & 1ULL) != 0) {
            result = usize_wrapping_mul(result, base);
        }
        exp >>= 1;
        if (exp > 0) {
            base = usize_wrapping_mul(base, base);
        }
    }
    return result;
}

uint64_t usize_saturating_pow(uint64_t base, uint64_t exp) {
    uint64_t result = 1;
    while (exp > 0) {
        if ((exp & 1ULL) != 0) {
            result = usize_saturating_mul(result, base);
        }
        exp >>= 1;
        if (exp > 0) {
            base = usize_saturating_mul(base, base);
        }
    }
    return result;
}

uint64_t usize_checked_pow(uint64_t base, uint64_t exp) {
    uint64_t result = 1;
    while (exp > 0) {
        if ((exp & 1ULL) != 0) {
            if (result != 0 && base != 0 && result > UINT64_MAX / base) {
                return 0;
            }
            result *= base;
        }
        exp >>= 1;
        if (exp > 0) {
            if (base != 0 && base > UINT64_MAX / base) {
                return 0;
            }
            base *= base;
        }
    }
    return result;
}

uint64_t usize_gcd(uint64_t left, uint64_t right) {
    while (right != 0) {
        uint64_t rem = left % right;
        left = right;
        right = rem;
    }
    return left;
}

uint64_t usize_lcm(uint64_t left, uint64_t right) {
    if (left == 0 || right == 0) {
        return 0;
    }
    return (left / usize_gcd(left, right)) * right;
}

int64_t int_gcd(int64_t left, int64_t right) {
    uint64_t a = left < 0 ? (uint64_t)(-left) : (uint64_t)left;
    uint64_t b = right < 0 ? (uint64_t)(-right) : (uint64_t)right;
    return (int64_t)usize_gcd(a, b);
}

int64_t int_lcm(int64_t left, int64_t right) {
    if (left == 0 || right == 0) {
        return 0;
    }
    uint64_t a = left < 0 ? (uint64_t)(-left) : (uint64_t)left;
    uint64_t b = right < 0 ? (uint64_t)(-right) : (uint64_t)right;
    return (int64_t)usize_lcm(a, b);
}

int int_is_even(int64_t value) {
    return (value % 2) == 0 ? 1 : 0;
}

int int_is_odd(int64_t value) {
    return (value % 2) != 0 ? 1 : 0;
}

int usize_is_even(uint64_t value) {
    return (value % 2ULL) == 0 ? 1 : 0;
}

int usize_is_odd(uint64_t value) {
    return (value % 2ULL) != 0 ? 1 : 0;
}

int usize_is_power_of_two(uint64_t value) {
    return value != 0 && (value & (value - 1ULL)) == 0 ? 1 : 0;
}

int int_is_power_of_two(int64_t value) {
    return value > 0 && usize_is_power_of_two((uint64_t)value);
}

uint64_t usize_next_power_of_two(uint64_t value) {
    if (value <= 1) {
        return 1;
    }
    value--;
    value |= value >> 1;
    value |= value >> 2;
    value |= value >> 4;
    value |= value >> 8;
    value |= value >> 16;
    value |= value >> 32;
    return value + 1;
}

uint64_t usize_checked_next_power_of_two(uint64_t value) {
    if (value > (1ULL << 63)) {
        return 0;
    }
    return usize_next_power_of_two(value);
}

uint64_t usize_saturating_next_power_of_two(uint64_t value) {
    if (value > (1ULL << 63)) {
        return UINT64_MAX;
    }
    return usize_next_power_of_two(value);
}

uint64_t usize_prev_power_of_two(uint64_t value) {
    if (value == 0) {
        return 0;
    }
    value |= value >> 1;
    value |= value >> 2;
    value |= value >> 4;
    value |= value >> 8;
    value |= value >> 16;
    value |= value >> 32;
    return value - (value >> 1);
}

int64_t int_prev_power_of_two(int64_t value) {
    if (value <= 0) {
        return 0;
    }
    return (int64_t)usize_prev_power_of_two((uint64_t)value);
}

int64_t int_next_power_of_two(int64_t value) {
    return int_checked_next_power_of_two(value);
}

int64_t int_checked_next_power_of_two(int64_t value) {
    if (value <= 0 || value > (INT64_MAX / 2) + 1) {
        return 0;
    }
    return (int64_t)usize_checked_next_power_of_two((uint64_t)value);
}

int64_t int_saturating_next_power_of_two(int64_t value) {
    if (value <= 0) {
        return 0;
    }
    if (value > (INT64_MAX / 2) + 1) {
        return INT64_MAX;
    }
    return (int64_t)usize_next_power_of_two((uint64_t)value);
}

uint64_t usize_align_up(uint64_t value, uint64_t alignment) {
    if (alignment <= 1) {
        return value;
    }
    return ((value + alignment - 1ULL) / alignment) * alignment;
}

int64_t int_align_up(int64_t value, int64_t alignment) {
    if (alignment <= 1) {
        return value;
    }
    return ((value + alignment - 1) / alignment) * alignment;
}

uint64_t usize_align_down(uint64_t value, uint64_t alignment) {
    if (alignment <= 1) {
        return value;
    }
    return (value / alignment) * alignment;
}

int64_t int_align_down(int64_t value, int64_t alignment) {
    if (alignment <= 1) {
        return value;
    }
    return (value / alignment) * alignment;
}

int64_t int_align_up_saturating(int64_t value, int64_t alignment) {
    int64_t remainder;
    int64_t delta;
    if (alignment <= 1) {
        return value;
    }
    remainder = value % alignment;
    if (remainder == 0) {
        return value;
    }
    delta = alignment - remainder;
    if (delta > 0 && INT64_MAX - value < delta) {
        return INT64_MAX;
    }
    return value + delta;
}

uint64_t usize_align_up_saturating(uint64_t value, uint64_t alignment) {
    uint64_t remainder;
    uint64_t delta;
    if (alignment <= 1) {
        return value;
    }
    remainder = value % alignment;
    if (remainder == 0) {
        return value;
    }
    delta = alignment - remainder;
    if (UINT64_MAX - value < delta) {
        return UINT64_MAX;
    }
    return value + delta;
}

uint64_t usize_div_ceil(uint64_t left, uint64_t right) {
    if (right == 0) {
        return 0;
    }
    uint64_t quotient = left / right;
    return (left % right) == 0 ? quotient : quotient + 1ULL;
}

int64_t int_signum(int64_t value) {
    if (value > 0) {
        return 1;
    }
    if (value < 0) {
        return -1;
    }
    return 0;
}

int int_is_positive(int64_t value) {
    return value > 0 ? 1 : 0;
}

int int_is_negative(int64_t value) {
    return value < 0 ? 1 : 0;
}

const char *platform_os(void) {
#if defined(_WIN32)
    return "windows";
#elif defined(__linux__)
    return "linux";
#elif defined(__APPLE__)
    return "macos";
#else
    return "unknown";
#endif
}

const char *platform_arch(void) {
#if defined(__x86_64__) || defined(_M_X64)
    return "x86_64";
#elif defined(__aarch64__) || defined(_M_ARM64)
    return "aarch64";
#elif defined(__i386__) || defined(_M_IX86)
    return "x86";
#elif defined(__arm__) || defined(_M_ARM)
    return "arm";
#else
    return "unknown";
#endif
}

int platform_path_separator(void) {
#if defined(_WIN32)
    return '\\';
#else
    return '/';
#endif
}

const char *platform_newline(void) {
#if defined(_WIN32)
    return "\r\n";
#else
    return "\n";
#endif
}

uint64_t cpu_count(void) {
#if defined(_WIN32)
    SYSTEM_INFO info;
    GetSystemInfo(&info);
    return info.dwNumberOfProcessors == 0 ? 1 : (uint64_t)info.dwNumberOfProcessors;
#else
    long count = sysconf(_SC_NPROCESSORS_ONLN);
    return count <= 0 ? 1 : (uint64_t)count;
#endif
}

uint64_t unix_time_secs(void) {
    time_t now = time(NULL);
    if (now < 0) {
        return 0;
    }
    return (uint64_t)now;
}

uint64_t unix_time_nanos(void) {
#if defined(_WIN32)
    FILETIME file_time;
    GetSystemTimeAsFileTime(&file_time);
    ULARGE_INTEGER ticks;
    ticks.LowPart = file_time.dwLowDateTime;
    ticks.HighPart = file_time.dwHighDateTime;
    return (ticks.QuadPart - 116444736000000000ULL) * 100ULL;
#else
    struct timeval now;
    if (gettimeofday(&now, NULL) != 0) {
        return 0;
    }
    return ((uint64_t)now.tv_sec * 1000000000ULL) + ((uint64_t)now.tv_usec * 1000ULL);
#endif
}

uint64_t unix_time_micros(void) {
    return unix_time_nanos() / 1000ULL;
}

uint64_t unix_time_millis(void) {
    return unix_time_nanos() / 1000000ULL;
}

uint64_t monotonic_nanos(void) {
#if defined(_WIN32)
    LARGE_INTEGER frequency;
    LARGE_INTEGER counter;
    if (!QueryPerformanceFrequency(&frequency) || !QueryPerformanceCounter(&counter) ||
        frequency.QuadPart <= 0) {
        return 0;
    }
    uint64_t seconds = (uint64_t)(counter.QuadPart / frequency.QuadPart);
    uint64_t remainder = (uint64_t)(counter.QuadPart % frequency.QuadPart);
    return (seconds * 1000000000ULL) + ((remainder * 1000000000ULL) / (uint64_t)frequency.QuadPart);
#else
    struct timespec now;
    if (clock_gettime(CLOCK_MONOTONIC, &now) != 0) {
        return 0;
    }
    return ((uint64_t)now.tv_sec * 1000000000ULL) + (uint64_t)now.tv_nsec;
#endif
}

uint64_t monotonic_micros(void) {
    return monotonic_nanos() / 1000ULL;
}

uint64_t monotonic_millis(void) {
    return monotonic_nanos() / 1000000ULL;
}

int sleep_millis(uint64_t ms) {
#if defined(_WIN32)
    while (ms > 0xFFFFFFFFULL) {
        Sleep(0xFFFFFFFFUL);
        ms -= 0xFFFFFFFFULL;
    }
    Sleep((DWORD)ms);
#else
    while (ms > 0) {
        uint64_t chunk = ms > 1000ULL ? 1000ULL : ms;
        usleep((useconds_t)(chunk * 1000ULL));
        ms -= chunk;
    }
#endif
    return 0;
}

const char *temp_dir(void) {
#if defined(_WIN32)
    const char *value = getenv("TMP");
    if (value == NULL || value[0] == '\0') {
        value = getenv("TEMP");
    }
    if (value == NULL || value[0] == '\0') {
        value = getenv("USERPROFILE");
    }
    return value == NULL ? "" : string_clone(value);
#else
    const char *value = getenv("TMPDIR");
    if (value == NULL || value[0] == '\0') {
        value = "/tmp";
    }
    return string_clone(value);
#endif
}

const char *home_dir(void) {
#if defined(_WIN32)
    const char *value = getenv("USERPROFILE");
    if (value != NULL && value[0] != '\0') {
        return string_clone(value);
    }
    const char *drive = getenv("HOMEDRIVE");
    const char *path = getenv("HOMEPATH");
    if (drive == NULL || drive[0] == '\0' || path == NULL || path[0] == '\0') {
        return "";
    }
    return string_concat(drive, path);
#else
    const char *value = getenv("HOME");
    return (value == NULL || value[0] == '\0') ? "" : string_clone(value);
#endif
}

const char *user_name(void) {
#if defined(_WIN32)
    const char *value = getenv("USERNAME");
#else
    const char *value = getenv("USER");
    if (value == NULL || value[0] == '\0') {
        value = getenv("LOGNAME");
    }
#endif
    return (value == NULL || value[0] == '\0') ? "" : string_clone(value);
}

static int geo_is_path_separator(char ch) {
    return ch == '/' || ch == '\\';
}

const char *path_join(const char *left, const char *right) {
    if (left == NULL || left[0] == '\0') {
        return string_clone(right);
    }
    if (right == NULL || right[0] == '\0') {
        return string_clone(left);
    }

    size_t left_len = strlen(left);
    while (*right != '\0' && geo_is_path_separator(*right)) {
        right++;
    }
    size_t right_len = strlen(right);
    int needs_separator = !geo_is_path_separator(left[left_len - 1]) && right_len > 0;
    size_t out_len = left_len + (needs_separator ? 1 : 0) + right_len;
    char *out = (char *)malloc(out_len + 1);
    if (out == NULL) {
        return "";
    }

    memcpy(out, left, left_len);
    size_t offset = left_len;
    if (needs_separator) {
        out[offset++] = (char)platform_path_separator();
    }
    memcpy(out + offset, right, right_len);
    out[out_len] = '\0';
    return out;
}

const char *path_file_name(const char *path) {
    if (path == NULL || path[0] == '\0') {
        return "";
    }
    size_t len = strlen(path);
    if (geo_is_path_separator(path[len - 1])) {
        return "";
    }
    size_t start = len;
    while (start > 0 && !geo_is_path_separator(path[start - 1])) {
        start--;
    }
    return string_clone(path + start);
}

const char *path_parent(const char *path) {
    if (path == NULL || path[0] == '\0') {
        return "";
    }
    size_t end = strlen(path);
    while (end > 0 && geo_is_path_separator(path[end - 1])) {
        end--;
    }
    while (end > 0 && !geo_is_path_separator(path[end - 1])) {
        end--;
    }
    if (end == 0) {
        return "";
    }
    while (end > 1 && geo_is_path_separator(path[end - 1])) {
        end--;
    }
    return geo_string_slice(path, end);
}

const char *path_extension(const char *path) {
    if (path == NULL || path[0] == '\0') {
        return "";
    }
    size_t len = strlen(path);
    if (geo_is_path_separator(path[len - 1])) {
        return "";
    }

    size_t component_start = len;
    while (component_start > 0 && !geo_is_path_separator(path[component_start - 1])) {
        component_start--;
    }

    size_t dot = len;
    while (dot > component_start && path[dot - 1] != '.') {
        dot--;
    }
    if (dot == component_start || dot == len) {
        return "";
    }
    return string_clone(path + dot);
}

const char *path_stem(const char *path) {
    if (path == NULL || path[0] == '\0') {
        return "";
    }
    size_t len = strlen(path);
    if (geo_is_path_separator(path[len - 1])) {
        return "";
    }

    size_t component_start = len;
    while (component_start > 0 && !geo_is_path_separator(path[component_start - 1])) {
        component_start--;
    }

    size_t end = len;
    while (end > component_start && path[end - 1] != '.') {
        end--;
    }
    if (end == component_start || end == len) {
        end = len;
    } else {
        end--;
    }
    return geo_string_slice(path + component_start, end - component_start);
}

int path_is_absolute(const char *path) {
    if (path == NULL || path[0] == '\0') {
        return 0;
    }
    if (path[0] == '/') {
        return 1;
    }
    if (geo_is_path_separator(path[0]) && geo_is_path_separator(path[1])) {
        return 1;
    }
    if (((path[0] >= 'A' && path[0] <= 'Z') || (path[0] >= 'a' && path[0] <= 'z')) &&
        path[1] == ':' && geo_is_path_separator(path[2])) {
        return 1;
    }
    return 0;
}

const char *path_without_extension(const char *path) {
    if (path == NULL || path[0] == '\0') {
        return "";
    }
    size_t len = strlen(path);
    if (geo_is_path_separator(path[len - 1])) {
        return string_clone(path);
    }

    size_t component_start = len;
    while (component_start > 0 && !geo_is_path_separator(path[component_start - 1])) {
        component_start--;
    }

    size_t dot = len;
    while (dot > component_start && path[dot - 1] != '.') {
        dot--;
    }
    if (dot == component_start || dot == len) {
        return string_clone(path);
    }
    return geo_string_slice(path, dot - 1);
}

const char *path_with_extension(const char *path, const char *extension) {
    const char *base = path_without_extension(path);
    if (extension == NULL || extension[0] == '\0') {
        return base;
    }

    const char *ext = extension;
    if (ext[0] == '.') {
        ext++;
    }

    size_t base_len = strlen(base);
    size_t ext_len = strlen(ext);
    char *out = (char *)malloc(base_len + 1 + ext_len + 1);
    if (out == NULL) {
        return "";
    }
    memcpy(out, base, base_len);
    out[base_len] = '.';
    memcpy(out + base_len + 1, ext, ext_len);
    out[base_len + 1 + ext_len] = '\0';
    return out;
}

static int geo_last_component_is_parent(const char *path, size_t len, size_t root_len) {
    if (len <= root_len) {
        return 0;
    }
    size_t start = len;
    while (start > root_len && !geo_is_path_separator(path[start - 1])) {
        start--;
    }
    return len - start == 2 && path[start] == '.' && path[start + 1] == '.';
}

static size_t geo_pop_path_component(char *path, size_t len, size_t root_len) {
    if (len <= root_len) {
        return len;
    }
    while (len > root_len && !geo_is_path_separator(path[len - 1])) {
        len--;
    }
    if (len > root_len && geo_is_path_separator(path[len - 1])) {
        len--;
    }
    return len < root_len ? root_len : len;
}

static size_t geo_append_path_component(
    char *out,
    size_t out_len,
    size_t root_len,
    const char *component,
    size_t component_len
) {
    if (out_len > root_len && !geo_is_path_separator(out[out_len - 1])) {
        out[out_len++] = (char)platform_path_separator();
    } else if (root_len > 0 && out_len == root_len && !geo_is_path_separator(out[out_len - 1]) &&
               !(root_len == 2 && out[1] == ':')) {
        out[out_len++] = (char)platform_path_separator();
    }
    memcpy(out + out_len, component, component_len);
    return out_len + component_len;
}

const char *path_normalize(const char *path) {
    if (path == NULL || path[0] == '\0') {
        return "";
    }

    size_t len = strlen(path);
    char *out = (char *)malloc(len + 2);
    if (out == NULL) {
        return "";
    }

    size_t pos = 0;
    size_t out_len = 0;
    size_t root_len = 0;
    char sep = (char)platform_path_separator();

    if (geo_is_path_separator(path[0]) && geo_is_path_separator(path[1])) {
        out[out_len++] = sep;
        out[out_len++] = sep;
        root_len = out_len;
        pos = 2;
        while (geo_is_path_separator(path[pos])) {
            pos++;
        }
    } else if (((path[0] >= 'A' && path[0] <= 'Z') || (path[0] >= 'a' && path[0] <= 'z')) &&
               path[1] == ':') {
        out[out_len++] = path[0];
        out[out_len++] = ':';
        pos = 2;
        if (geo_is_path_separator(path[pos])) {
            out[out_len++] = sep;
            pos++;
            while (geo_is_path_separator(path[pos])) {
                pos++;
            }
        }
        root_len = out_len;
    } else if (geo_is_path_separator(path[0])) {
        out[out_len++] = sep;
        root_len = out_len;
        pos = 1;
        while (geo_is_path_separator(path[pos])) {
            pos++;
        }
    }

    while (pos < len) {
        while (geo_is_path_separator(path[pos])) {
            pos++;
        }
        size_t start = pos;
        while (pos < len && !geo_is_path_separator(path[pos])) {
            pos++;
        }
        size_t component_len = pos - start;
        if (component_len == 0 || (component_len == 1 && path[start] == '.')) {
            continue;
        }
        if (component_len == 2 && path[start] == '.' && path[start + 1] == '.') {
            if (out_len > root_len && !geo_last_component_is_parent(out, out_len, root_len)) {
                out_len = geo_pop_path_component(out, out_len, root_len);
            } else if (root_len == 0) {
                out_len = geo_append_path_component(out, out_len, root_len, path + start, component_len);
            }
            continue;
        }
        out_len = geo_append_path_component(out, out_len, root_len, path + start, component_len);
    }

    if (out_len == 0) {
        out[out_len++] = '.';
    }
    out[out_len] = '\0';
    return out;
}

static const char *geo_path_convert_separator(const char *path, char separator) {
    if (path == NULL) {
        return "";
    }
    size_t len = strlen(path);
    char *out = (char *)malloc(len + 1);
    if (out == NULL) {
        return "";
    }
    for (size_t i = 0; i < len; i++) {
        out[i] = geo_is_path_separator(path[i]) ? separator : path[i];
    }
    out[len] = '\0';
    return out;
}

const char *path_to_unix(const char *path) {
    return geo_path_convert_separator(path, '/');
}

const char *path_to_windows(const char *path) {
    return geo_path_convert_separator(path, '\\');
}

const char *current_dir(void) {
    size_t capacity = 256;
    for (;;) {
        char *buffer = (char *)malloc(capacity);
        if (buffer == NULL) {
            return "";
        }
        if (geo_getcwd(buffer, (int)capacity) != NULL) {
            return buffer;
        }
        free(buffer);
        if (capacity > (1u << 20)) {
            return "";
        }
        capacity *= 2;
    }
}

const char *path_absolute(const char *path) {
    if (path == NULL || path[0] == '\0') {
        return "";
    }
    if (path_is_absolute(path)) {
        return path_normalize(path);
    }
    const char *cwd = current_dir();
    if (cwd == NULL || cwd[0] == '\0') {
        return "";
    }
    return path_normalize(path_join(cwd, path));
}

int change_dir(const char *path) {
    if (path == NULL) {
        return 1;
    }
    return geo_chdir(path) == 0 ? 0 : 1;
}
