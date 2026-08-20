import QtQuick
import Quickshell
import Quickshell.Io

QtObject {
	id: root

	property string path: Quickshell.env("YOBU_SOCKET")
		?? ((Quickshell.env("XDG_RUNTIME_DIR") ?? "/tmp") + "/yobu.sock")
	property string clientName: "quickshell"
	property var capabilities: []
	property bool active: true
	property int reconnectInterval: 1000
	readonly property var socket: socketLoader.item
	readonly property bool connected: socket !== null && socket.connected

	property real nextRequestId: 1
	property var requests: ({})
	property var subscriptions: ({})
	property bool handshaken: false

	signal eventReceived(string topic, var payload)
	signal protocolError(string code, string message, var requestId)
	signal socketError(var error)

	onActiveChanged: {
		reconnectTimer.stop()
		reconnectKick.stop()
		socketLoader.active = active
		if (!active)
			handleDisconnected()
	}

	onConnectedChanged: {
		if (connected)
			handleConnected()
		else
			handleDisconnected()
	}

	onPathChanged: restartConnection()

	function socketWrite(message) {
		if (!connected) return false
		socket.write(JSON.stringify(message) + "\n")
		socket.flush()
		return true
	}

	function request(method, params = {}, onResult = () => {}, onError = () => {}) {
		const id = nextRequestId++
		if (!connected) {
			onError("connection_unavailable", "daemon is not connected")
			return -1
		}

		requests[id] = {
			resolve: onResult,
			reject: onError
		}
		socketWrite({ type: "request", id, method, params })
		return id
	}

	function subscribe(topic, handler = null) {
		let handlers = subscriptions[topic]
		const firstSubscription = handlers === undefined
		if (firstSubscription) {
			handlers = []
			subscriptions[topic] = handlers
		}
		if (handler !== null && !handlers.includes(handler))
			handlers.push(handler)
		if (firstSubscription && handshaken)
			socketWrite({ type: "subscribe", topic })

		return () => root.unsubscribe(topic, handler)
	}

	function unsubscribe(topic, handler = null) {
		const handlers = subscriptions[topic]
		if (handlers === undefined) return

		if (handler === null) {
			delete subscriptions[topic]
		} else {
			const index = handlers.indexOf(handler)
			if (index !== -1)
				handlers.splice(index, 1)
			if (handlers.length > 0) return
			delete subscriptions[topic]
		}

		if (handshaken)
			socketWrite({ type: "unsubscribe", topic })
	}

	function failPendingRequests(code, message) {
		const pending = requests
		requests = ({})
		Object.keys(pending).forEach(id => {
			try {
				pending[id].reject(code, message)
			} catch (error) {
				console.error("DaemonConnection request error handler failed:", error)
			}
		})
	}

	function scheduleReconnect() {
		if (active)
			reconnectTimer.restart()
	}

	function recreateSocket() {
		socketLoader.active = false
		if (active)
			reconnectKick.restart()
	}

	function restartConnection() {
		reconnectTimer.stop()
		recreateSocket()
	}

	function handleConnected() {
		if (handshaken) return
		handshaken = true
		socketWrite({
			type: "hello",
			name: clientName,
			capabilities
		})
		Object.keys(subscriptions).forEach(topic => {
			socketWrite({ type: "subscribe", topic })
		})
	}

	function handleDisconnected() {
		if (!handshaken) return
		handshaken = false
		failPendingRequests("connection_lost", "daemon connection was lost")
	}

	function handleMessage(message) {
		switch (message.type) {
		case "response": {
			const pending = requests[message.id]
			if (pending === undefined) {
				console.warn("DaemonConnection received an unknown response id:", message.id)
				return
			}
			delete requests[message.id]
			try {
				pending.resolve(message.result)
			} catch (error) {
				console.error("DaemonConnection response handler failed:", error)
			}
			break
		}
		case "error": {
			const pending = requests[message.id]
			if (pending !== undefined) {
				delete requests[message.id]
				try {
					pending.reject(message.code, message.message)
				} catch (error) {
					console.error("DaemonConnection request error handler failed:", error)
				}
			}
			protocolError(message.code, message.message, message.id)
			break
		}
		case "event": {
			eventReceived(message.topic, message.payload)
			const handlers = subscriptions[message.topic] ?? []
			handlers.slice().forEach(handler => {
				try {
					handler(message.payload)
				} catch (error) {
					console.error("DaemonConnection subscription handler failed:", error)
				}
			})
			break
		}
		default:
			console.warn("DaemonConnection received an unknown message type:", message.type)
		}
	}

	property Timer reconnectTimer: Timer {
		interval: root.reconnectInterval
		repeat: false
		onTriggered: root.recreateSocket()
	}

	property Timer reconnectKick: Timer {
		interval: 50
		repeat: false
		onTriggered: {
			if (root.active)
				socketLoader.active = true
		}
	}

	property Component socketComponent: Component {
		Socket {
			path: root.path
			connected: true

			onConnectedChanged: {
				if (!connected)
					root.scheduleReconnect()
			}
			onError: error => {
				root.socketError(error)
				root.scheduleReconnect()
			}
			Component.onDestruction: connected = false

			parser: SplitParser {
				onRead: data => {
					try {
						root.handleMessage(JSON.parse(data))
					} catch (error) {
						console.error("DaemonConnection received invalid JSON:", error)
					}
				}
			}
		}
	}

	property Loader socketLoader: Loader {
		active: root.active
		sourceComponent: root.socketComponent
	}
}
