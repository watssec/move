FROM ubuntu:18.04


RUN apt-get update
RUN cd diem && ./script/dev_setup.sh -ypt
RUN source ~/.profile
RUN cargo install --path diem-move/df-cli
RUN cargo install --path language/move-analyzer
