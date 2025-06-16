FROM public.ecr.aws/amazonlinux/amazonlinux:latest
RUN yum install -y tar \
  gzip \
  git \
  findutils # findutils needed for gradle/xargs

# Download and extract Java JDK 17
# https://docs.aws.amazon.com/corretto/latest/corretto-17-ug/downloads-list.html
ARG JDK_VERSION=17.0.10.8.1

ARG JDK_MV=17

# or aarch64 on arm machines
ARG JDK_ARCH=x64 

COPY run.gradle /tmp

RUN cd /tmp && \
    curl -LO https://corretto.aws/downloads/latest/amazon-corretto-17-${JDK_ARCH}-linux-jdk.tar.gz && \
    tar -xzf amazon-corretto-17-${JDK_ARCH}-linux-jdk.tar.gz  -C /usr/local/ && \
    rm amazon-corretto-17-${JDK_ARCH}-linux-jdk.tar.gz && \
    export JAVA_HOME=/usr/local/amazon-corretto-${JDK_VERSION}-linux-${JDK_ARCH} && \
    export PATH=$PATH:/usr/local/amazon-corretto-${JDK_VERSION}-linux-${JDK_ARCH}/bin && \
    git clone https://github.com/opensearch-project/OpenSearch.git && \
    cd OpenSearch && \
    cp /tmp/run.gradle ./gradle/run.gradle && \
    cd /tmp && mv /tmp/OpenSearch /usr/lib


ENV JAVA_HOME=/usr/local/amazon-corretto-${JDK_VERSION}-linux-${JDK_ARCH}
ENV PATH=$PATH:/usr/local/amazon-corretto-${JDK_VERSION}-linux-${JDK_ARCH}/bin

CMD ["/usr/lib/OpenSearch/gradlew", "run"]
