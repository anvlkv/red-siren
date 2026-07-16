import { createContext, PropsWithChildren, useContext } from "react";

export interface AppThemeContextValue {
    backgroundColor: string;
    primaryColor: string;
    secondaryColor: string;
    tertiaryColor: string;
    isDark: boolean;
    pixelRatio: number;
    width: number;
    height: number;
}

const AppThemeContext = createContext<AppThemeContextValue | null>(null);

function ThemeProvider({
    children,
    backgroundColor,
    primaryColor,
    secondaryColor,
    tertiaryColor,
    isDark,
    pixelRatio,
    width,
    height,
}: PropsWithChildren<AppThemeContextValue>) {
    return (
        <AppThemeContext.Provider
            value={{
                backgroundColor,
                primaryColor,
                secondaryColor,
                tertiaryColor,
                isDark,
                pixelRatio,
                width,
                height,
            }}
        >
            {children}
        </AppThemeContext.Provider>
    );
}

export default ThemeProvider;

export function useAppTheme() {
    const context = useContext(AppThemeContext)!;
    return context;
}
