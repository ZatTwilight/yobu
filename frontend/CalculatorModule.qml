import QtQuick

QtObject {
	id: root
	required property DaemonConnection daemon

	function evaluate(query, onResult, onError = () => {}) {
		if (!daemon.connected) {
			onError("connection_unavailable", "daemon not connected")
			return
		}
		daemon.request(
			"calculator.evaluate",
			{ query },
			result => onResult(result.entry ?? null),
			onError
		)
	}
}
