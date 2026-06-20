import { useDarkMode, useWindowSize } from "@reactuses/core";
import { invoke } from "@tauri-apps/api/core";
import classnames from "classnames";
import { useEffect } from "react";
import { Outlet, useLocation } from "react-router";
import Menu from "../components/Menu";
import "./App.css";

function App() {
  const [isDark, toggleDark] = useDarkMode({
    classNameDark: "dark",
    classNameLight: "light",
    defaultValue: false,
  });
  const { width, height } = useWindowSize();

  const className = classnames("App", {
    dark: isDark,
    light: !isDark,
  });
  const location = useLocation();

  useEffect(() => {
    (async () => {
      await invoke("update_window_appearance", { width, height, isDark });
    })()
  }, [width, height, isDark]);

  return (
    <main className={className}>
      <Outlet />
      <aside>
        <Menu />
      </aside>
    </main>
  );
}

export default App;
