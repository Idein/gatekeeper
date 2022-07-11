
variable "GATEKEEPER_VER" {
	# latest
	# or <major>
	# or <major>.<minor>
	# or <major>.<minor>.<patch>
	default = "latest"
}

group "default" {
	targets = ["x86_64", "arm32v6", "arm32v7"]
}

target "x86_64" {
	dockerfile = "Dockerfile.x86_64"
	args = {
		GATEKEEPER_VER = "${GATEKEEPER_VER}"
	}
	tags = ["docker.io/idein/gatekeeper:${GATEKEEPER_VER}-x86_64"]
	platforms = ["linux/amd64"]
	output = ["type=docker"]
}

target "arm32v6" {
	dockerfile = "Dockerfile.arm32v6"
	args = {
		GATEKEEPER_VER = "${GATEKEEPER_VER}"
	}
	tags = ["docker.io/idein/gatekeeper:${GATEKEEPER_VER}-arm32v6"]
	platforms = ["linux/arm/v6"]
	output = ["type=docker"]
}

target "arm32v7" {
	dockerfile = "Dockerfile.arm32v7"
	args = {
		GATEKEEPER_VER = "${GATEKEEPER_VER}"
	}
	tags = ["docker.io/idein/gatekeeper:${GATEKEEPER_VER}-arm32v7"]
	platforms = ["linux/arm/v7"]
	output = ["type=docker"]
}

