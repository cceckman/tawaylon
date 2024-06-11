
set -eux 

BUILDROOT="$(pwd)"
git clone -b vosk --single-branch https://github.com/alphacep/kaldi 
cd $BUILDROOT/kaldi/tools 
git clone -b v0.3.20 --single-branch https://github.com/xianyi/OpenBLAS 
git clone -b v3.2.1  --single-branch https://github.com/alphacep/clapack 
make -C OpenBLAS ONLY_CBLAS=1 DYNAMIC_ARCH=1 TARGET=NEHALEM USE_LOCKING=1 USE_THREAD=0 all 
make -C OpenBLAS PREFIX=$(pwd)/OpenBLAS/install install 
find . -name "*.a" | xargs cp -t ../../OpenBLAS/install/lib 
cd $BUILDROOT/kaldi/tools 
git clone --single-branch https://github.com/alphacep/openfst openfst 
cd openfst 
autoreconf -i 
CFLAGS="-g -O3" ./configure --prefix=$BUILDROOT/kaldi/tools/openfst --enable-static --enable-shared --enable-far --enable-ngram-fsts --enable-lookahead-fsts --with-pic --disable-bin 
make install 
cd $BUILDROOT/kaldi/src 
./configure --mathlib=OPENBLAS_CLAPACK --shared --use-cuda=no 
sed -i 's:-msse -msse2:-msse -msse2:g' kaldi.mk 
sed -i 's: -O1 : -O3 :g' kaldi.mk 
make -j $(nproc) online2 rnnlm 
find $BUILDROOT/kaldi -name "*.o" -exec rm {} \;

