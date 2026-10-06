#include <assert.h>
#include <limits.h>
#include <string.h>
#include STB_VORBIS_SOURCE

int main(void) {
    unsigned char data[16] = {0};
    stb_vorbis f;
    memset(&f, 0, sizeof(f));
    f.stream = f.stream_start = data;
    f.stream_end = data + sizeof(data);
    unsigned int valid[] = {0, 1, sizeof(data) - 1};
    for (unsigned int i = 0; i < sizeof(valid) / sizeof(valid[0]); ++i) {
        assert(set_file_offset(&f, valid[i]) == 1);
        assert(f.stream == data + valid[i]);
        assert(f.eof == 0);
    }
    unsigned int invalid[] = {sizeof(data), sizeof(data) + 1, INT_MAX, UINT_MAX};
    for (unsigned int i = 0; i < sizeof(invalid) / sizeof(invalid[0]); ++i) {
        assert(set_file_offset(&f, invalid[i]) == 0);
        assert(f.stream == f.stream_end);
        assert(f.eof == 1);
    }
    f.stream_end = f.stream_start;
    assert(set_file_offset(&f, 0) == 0);
    f.push_mode = 1;
    assert(set_file_offset(&f, 0) == 0);
    return 0;
}
