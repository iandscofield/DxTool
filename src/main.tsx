import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";

// StrictMode is left off on purpose: it runs startup effects twice in development,
// which would read the whole hardware inventory twice.
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(<App />);
