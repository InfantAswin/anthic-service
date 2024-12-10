# Use an official Rust image as the base
FROM rust:1.81

# Install necessary system dependencies
RUN apt-get update && \
    apt-get install -y \
        pkg-config \
        libudev-dev \
        libssl-dev \
        sudo \
        curl

# Install Rustup and set up the Rust environment
RUN curl --proto '=https' --tlsv1.3 https://sh.rustup.rs -sSf | sh -s -- -y

# Add the Rust tools to PATH
ENV PATH="/root/.cargo/bin:${PATH}"

# Set the working directory in the container
WORKDIR /app

# Copy the current directory contents into the container at /app
COPY . .

# Compile the Rust application
RUN cargo build --release

# Expoese 8080
EXPOSE 8080

# Run the compiled binary by default
CMD ["./target/release/anthic-service"]