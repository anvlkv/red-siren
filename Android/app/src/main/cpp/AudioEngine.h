
#ifndef PART1_AUDIOENGINE_H
#define PART1_AUDIOENGINE_H

#include <aaudio/AAudio.h>


class AudioEngine {
public:
    bool start();

    void stop();

    void restart();

private:
    AAudioStream *stream_;
};


#endif //PART1_AUDIOENGINE_H